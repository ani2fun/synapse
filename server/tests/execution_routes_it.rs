//! Integration: POST /api/run through the REAL stack — router → service → `GoJudgeRunner` →
//! wire — against a LOCAL go-judge stub (an axum server speaking the go-judge protocol), so
//! the whole adapter path is exercised without a sandbox.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::routing::post;
use serde_json::Value;
use tower::ServiceExt;

/// A go-judge lookalike: always answers `response`, counts hits. Returns its base URL.
async fn stub_go_judge(response: &'static str, hits: Arc<AtomicUsize>) -> String {
    let app = Router::new().route(
        "/run",
        post(move || {
            hits.fetch_add(1, Ordering::SeqCst);
            async move { ([(header::CONTENT_TYPE, "application/json")], response) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

async fn post_run(app: Router, body: Value) -> (StatusCode, Value) {
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/run")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

const ACCEPTED: &str = r#"[{"status":"Accepted","exitStatus":0,"time":12000000,"memory":5632000,
  "files":{"stdout":"42\n","stderr":""}}]"#;
const CRASHED: &str = r#"[{"status":"Nonzero Exit Status","exitStatus":1,
  "files":{"stdout":"","stderr":"Traceback: boom"}}]"#;

fn run_body(language: &str, source: &str) -> Value {
    serde_json::json!({ "language": language, "source": source })
}

#[tokio::test]
async fn a_good_run_is_200_with_the_wire_result() {
    let hits = Arc::new(AtomicUsize::new(0));
    let tmp = tempfile::tempdir().unwrap();
    let url = stub_go_judge(ACCEPTED, Arc::clone(&hits)).await;
    let app = common::app_with_executor(tmp.path(), &url);

    let (status, json) = post_run(app, run_body("py", "print(42)")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "Accepted");
    assert_eq!(json["stdout"], "42\n");
    assert_eq!(json["timeSeconds"], 0.012);
    assert_eq!(json["memoryKb"], 5500);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_badly_running_program_is_still_200() {
    let tmp = tempfile::tempdir().unwrap();
    let url = stub_go_judge(CRASHED, Arc::new(AtomicUsize::new(0))).await;
    let (status, json) = post_run(
        common::app_with_executor(tmp.path(), &url),
        run_body("py", "boom()"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "RuntimeError");
    assert_eq!(json["stderr"], "Traceback: boom");
}

#[tokio::test]
async fn unknown_languages_are_422_and_never_reach_the_backend() {
    let hits = Arc::new(AtomicUsize::new(0));
    let tmp = tempfile::tempdir().unwrap();
    let url = stub_go_judge(ACCEPTED, Arc::clone(&hits)).await;
    let (status, json) = post_run(
        common::app_with_executor(tmp.path(), &url),
        run_body("cobol", "DISPLAY 'HI'"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["error"], "Language 'cobol' is not runnable");
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn oversized_payloads_are_413() {
    let tmp = tempfile::tempdir().unwrap();
    let big = "x".repeat(64 * 1024 + 1);
    let (status, json) = post_run(common::app_over(tmp.path()), run_body("py", &big)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(json["error"], "Source too large");
}

#[tokio::test]
async fn a_dead_backend_is_503_with_the_operator_hint() {
    let tmp = tempfile::tempdir().unwrap();
    // common::app_over points the executor at a refusing port.
    let (status, json) = post_run(common::app_over(tmp.path()), run_body("py", "print(1)")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json["error"], "Execution backend unavailable");
    assert_eq!(json["hint"], "Is go-judge running? Set EXECUTOR_URL.");
}

#[tokio::test]
async fn an_unintelligible_backend_is_502() {
    let tmp = tempfile::tempdir().unwrap();
    let url = stub_go_judge("<html>not json</html>", Arc::new(AtomicUsize::new(0))).await;
    let (status, json) = post_run(
        common::app_with_executor(tmp.path(), &url),
        run_body("py", "print(1)"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(json["error"], "Execution backend failed");
}
