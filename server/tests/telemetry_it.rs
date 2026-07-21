//! Integration: request tracing through the REAL assembled router.
//!
//! The server is designed to emit `route→service→adapter` spans for every request. These
//! tests hold that design to the actual output: a span is emitted per request, it carries the
//! fields needed to correlate a production failure, and the id round-trips on the response.
//!
//! The subscriber writes into a shared buffer rather than asserting on a mock, because what
//! matters is what an operator reading logs actually sees.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::io;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use tracing_subscriber::layer::SubscriberExt;

/// A `MakeWriter` that appends everything to one buffer we can read back.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Capture {
    fn contents(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl io::Write for Capture {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn app() -> axum::Router {
    common::app_over(std::path::Path::new("/nonexistent-synapse-content"))
}

/// Drive one request with a capturing subscriber installed for the duration.
///
/// `set_default` rather than `init`: a global subscriber can only be installed once per
/// process, and these tests share one with every other test in the binary.
async fn traced(request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_writer(capture.clone())
            .with_ansi(false)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE),
    );
    let response = {
        let _guard = tracing::subscriber::set_default(subscriber);
        app().oneshot(request).await.unwrap()
    };
    (response.status(), response.headers().clone(), capture.contents())
}

#[tokio::test]
async fn a_request_emits_an_http_span_with_method_path_status_and_latency() {
    let (status, _headers, logs) =
        traced(Request::builder().uri("/api/health").body(Body::empty()).unwrap()).await;

    assert_eq!(status, StatusCode::OK);
    assert!(logs.contains("http"), "no http span in output:\n{logs}");
    assert!(logs.contains("method=GET"), "method missing:\n{logs}");
    assert!(logs.contains("/api/health"), "path missing:\n{logs}");
    // The two fields that make a span worth having: what happened, and how long it took.
    assert!(logs.contains("status=200"), "status missing:\n{logs}");
    assert!(logs.contains("latency_ms"), "latency missing:\n{logs}");
}

/// The correlation guarantee. Without an id on both the span and the response, a user
/// reporting "it broke" cannot be joined to the log line that recorded the break — which is
/// the entire reason request-id propagation exists.
#[tokio::test]
async fn the_request_id_is_generated_echoed_on_the_response_and_recorded_on_the_span() {
    let (_status, headers, logs) =
        traced(Request::builder().uri("/api/health").body(Body::empty()).unwrap()).await;

    let id = headers
        .get("x-request-id")
        .expect("x-request-id must be echoed on the response")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(!id.is_empty(), "generated id must not be blank");
    assert!(
        logs.contains(&id),
        "the response id {id} does not appear on the span:\n{logs}"
    );
}

/// A caller-supplied id must WIN rather than being replaced, or a trace cannot be followed
/// across a hop that already had one.
#[tokio::test]
async fn a_caller_supplied_request_id_is_preserved_end_to_end() {
    let supplied = "cutover-rehearsal-12345";
    let (_status, headers, logs) = traced(
        Request::builder()
            .uri("/api/health")
            .header("x-request-id", supplied)
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(
        headers.get("x-request-id").unwrap().to_str().unwrap(),
        supplied,
        "a supplied id must be preserved, not regenerated"
    );
    assert!(
        logs.contains(supplied),
        "the supplied id must reach the span:\n{logs}"
    );
}

/// Cardinality guard, stated precisely.
///
/// The invariant is about the `http` span specifically: its `path` is the MATCHED ROUTE, so
/// the top-level aggregation key stays bounded no matter how large the content library gets.
/// It is NOT that the concrete path may never appear anywhere — the `catalog.lesson` child
/// span deliberately carries the real path, because when you are reading a trace to find out
/// why one lesson 500s, that is the field you need. Bounded key at the top, specifics on the
/// children.
///
/// This test asserts on the `http{…}` prelude alone, so a future high-cardinality field added
/// to the ROUTE span fails while useful detail on child spans stays legal.
#[tokio::test]
async fn the_http_span_path_is_the_matched_route_not_the_concrete_uri() {
    let (_status, _headers, logs) = traced(
        Request::builder()
            .uri("/api/synapse/lesson/some/deep/lesson/path")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    // Every `http{ ... }` field set that appears in the captured output.
    let http_spans: Vec<&str> = logs
        .match_indices("http{")
        .filter_map(|(start, _)| {
            let rest = &logs[start..];
            rest.find('}').map(|end| &rest[..=end])
        })
        .collect();
    assert!(!http_spans.is_empty(), "no http span found:\n{logs}");

    for span in &http_spans {
        assert!(
            !span.contains("some/deep/lesson"),
            "the concrete path leaked into the ROUTE span — cardinality guard broken: {span}"
        );
        assert!(
            span.contains("path=/api/synapse/"),
            "the route span should record the matched route: {span}"
        );
    }
}
