//! Integration: the submission endpoints that need NO database — the store is never touched on
//! these paths. The full 202 → judge → poll round trip lives in the gated `postgres_it` suite.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

async fn call(app: axum::Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(json) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };
    let res = app.oneshot(builder.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn submitting_to_a_non_problem_is_404_and_touches_no_store() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join("01-dsa/book.json"), "{}");
    write(&tmp.path().join("01-dsa/01-intro.md"), "prose, no suite");
    let (status, body) = call(
        common::app_over(tmp.path()),
        "POST",
        "/api/submissions",
        Some(json!({ "path": ["dsa", "intro"], "language": "py", "source": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Not a problem");
}

#[tokio::test]
async fn a_malformed_id_is_400_before_any_lookup() {
    let tmp = tempfile::tempdir().unwrap();
    let (status, body) = call(
        common::app_over(tmp.path()),
        "GET",
        "/api/submissions/not-a-uuid",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "Not a submission id");
}

#[tokio::test]
async fn an_invalid_authored_suite_is_a_loud_500() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join("01-dsa/book.json"), "{}");
    write(&tmp.path().join("01-dsa/01-two-sum.md"), "prose");
    write(&tmp.path().join("01-dsa/01-two-sum.tests.json"), "not json");
    let (status, body) = call(
        common::app_over(tmp.path()),
        "POST",
        "/api/submissions",
        Some(json!({ "path": ["dsa", "two-sum"], "language": "py", "source": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"], "The authored suite is invalid");
}
