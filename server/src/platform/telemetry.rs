//! Request tracing — carries a request's identity across the `route → service → adapter` hops.
//!
//! Before this the server emitted 48 flat events with no span context: a 500 in production
//! could not be tied back to the request that caused it, because nothing carried a request
//! identity across the route → service → adapter hops. `tracing` was a dependency and a
//! logging macro, not an observability story.
//!
//! The route hop lives here. The service and adapter hops are `#[instrument]` attributes at
//! their own layers, which nest inside this span automatically — that is the entire reason
//! spans beat structured events: the child does not have to know the parent's fields.

use axum::extract::MatchedPath;
use axum::http::Request;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::Level;

/// `x-request-id` — set on the way in if the caller did not supply one, echoed on the way
/// out so a user reporting a failure can quote the id from their own response headers.
const REQUEST_ID: &str = "x-request-id";

/// The per-request span every other span nests under.
///
/// The path field is the MATCHED ROUTE (`/api/synapse/lesson/{*path}`), never the raw URI.
/// A raw URI would make every lesson its own span name — unbounded cardinality, which is
/// what turns tracing from an asset into a bill. The concrete path still rides on the
/// request; it is the aggregation key that has to stay bounded.
fn make_span<B>(request: &Request<B>) -> tracing::Span {
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| request.uri().path().to_owned(), |m| m.as_str().to_owned());
    let request_id = request
        .headers()
        .get(REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    tracing::info_span!(
        "http",
        method = %request.method(),
        path = %path,
        request_id = %request_id,
        // Filled in by TraceLayer's on_response — declared here so the field exists for the
        // whole span rather than appearing only on the closing event.
        status = tracing::field::Empty,
        latency_ms = tracing::field::Empty,
    )
}

/// Wrap a router in the tracing group.
///
/// This applies the layers rather than returning them because the `on_response` /
/// `on_failure` closures give `TraceLayer` an unnameable type — and hiding that behind a
/// function keeps the ordering rule in one place instead of at every call site.
///
/// Ordering is load-bearing and `.layer()` applies BOTTOM-UP, so the listing below reads in
/// reverse of execution: set-id runs first, then trace, then propagate. `SetRequestIdLayer`
/// must run before `TraceLayer` or the span is built from a request that has no id yet and
/// `request_id` is empty on every span — which looks fine until you try to correlate a
/// failure.
pub fn apply(router: axum::Router) -> axum::Router {
    let set = SetRequestIdLayer::new(axum::http::HeaderName::from_static(REQUEST_ID), MakeRequestUuid);
    let trace = TraceLayer::new_for_http()
        .make_span_with(make_span as fn(&Request<axum::body::Body>) -> tracing::Span)
        .on_response(
            |response: &axum::http::Response<axum::body::Body>,
             latency: std::time::Duration,
             span: &tracing::Span| {
                span.record("status", response.status().as_u16());
                // Milliseconds as f64: sub-millisecond handlers are the common case here
                // (the catalog cache serves in microseconds) and integer ms would render
                // the entire fast path as 0.
                span.record("latency_ms", latency.as_secs_f64() * 1_000.0);
                tracing::event!(Level::INFO, "response");
            },
        )
        .on_failure(
            |error: tower_http::classify::ServerErrorsFailureClass,
             latency: std::time::Duration,
             _span: &tracing::Span| {
                tracing::event!(
                    Level::WARN,
                    %error,
                    latency_ms = latency.as_secs_f64() * 1_000.0,
                    "request failed"
                );
            },
        );
    let propagate = PropagateRequestIdLayer::new(axum::http::HeaderName::from_static(REQUEST_ID));
    router.layer(propagate).layer(trace).layer(set)
}
