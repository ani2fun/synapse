//! Integration: the tutor surface (oracle: `TutorRoutesSpec` + the end-to-end 404 pin) — the
//! structural-404 mount gating, the error mapping, and the always-answering config, through
//! the REAL router over a fake client.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::Value;
use synapse_server::tutoring::application::{TutorClient, TutorError, TutoringService};
use synapse_server::tutoring::http::{TutorRoutesState, routes};
use synapse_shared::tutor::ChatMessage;
use tower::ServiceExt;

/// Scripted client: one canned outcome for every call.
struct Scripted(Result<String, TutorError>);

impl TutorClient for Scripted {
    async fn chat(&self, _prompt: &str, _history: &[ChatMessage]) -> Result<String, TutorError> {
        self.0.clone()
    }
}

fn app(enabled: bool, outcome: Result<String, TutorError>) -> axum::Router {
    routes(TutorRoutesState {
        service: Arc::new(TutoringService::new(Scripted(outcome))),
        enabled,
        model: "llama3.1".to_owned(),
    })
}

async fn call(app: axum::Router, method: &str, uri: &str, body: Option<&str>) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_owned()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let res = app.oneshot(request).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

const CHAT_BODY: &str = r#"{"messages":[{"role":"user","content":"I'm stuck"}]}"#;

#[tokio::test]
async fn config_always_answers_enabled_or_not() {
    let (status, body) = call(app(true, Ok("hi".into())), "GET", "/api/tutor/config", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], true);
    assert_eq!(body["model"], "llama3.1");

    let (status, body) = call(app(false, Ok("hi".into())), "GET", "/api/tutor/config", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], false);
}

#[tokio::test]
async fn enabled_chat_delegates_and_returns_the_reply() {
    let (status, body) = call(
        app(true, Ok("try two pointers".into())),
        "POST",
        "/api/tutor/chat",
        Some(CHAT_BODY),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["content"], "try two pointers");
}

#[tokio::test]
async fn backend_unavailable_is_503_and_failed_is_502() {
    let (status, body) = call(
        app(true, Err(TutorError::BackendUnavailable("down".into()))),
        "POST",
        "/api/tutor/chat",
        Some(CHAT_BODY),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body["detail"].as_str().unwrap().contains("down"));
    assert!(body["hint"].as_str().unwrap().contains("TUTOR_URL"));

    let (status, _) = call(
        app(true, Err(TutorError::BackendFailed("boom".into()))),
        "POST",
        "/api/tutor/chat",
        Some(CHAT_BODY),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn disabled_chat_is_a_structural_404() {
    // The route is never MOUNTED — not a handler check.
    let (status, _) = call(
        app(false, Ok("hi".into())),
        "POST",
        "/api/tutor/chat",
        Some(CHAT_BODY),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_assembled_dev_app_serves_config_off_and_404s_chat() {
    // Through the FULL app (common defaults = coach off), end to end.
    let tmp = tempfile::tempdir().unwrap();
    let full = common::app_over(tmp.path());
    let (status, body) = call(full.clone(), "GET", "/api/tutor/config", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], false);
    let (status, _) = call(full, "POST", "/api/tutor/chat", Some(CHAT_BODY)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
