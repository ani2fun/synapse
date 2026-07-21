//! Integration: the run/submit budget through the REAL router — the 429 envelope, per-IP
//! keying via X-Forwarded-For, and the sign-in hint (the unit windows live in
//! `rate_limiter_tests`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use synapse_server::platform::rate_limiter::{RateLimitBucket, RateLimiter};
use tower::ServiceExt;

fn tiny_budget_app(root: &std::path::Path) -> axum::Router {
    let mut deps = common::deps(root);
    deps.limiter = std::sync::Arc::new(RateLimiter::new(
        RateLimitBucket {
            window_seconds: 3600,
            limit: 2,
        },
        RateLimitBucket {
            window_seconds: 3600,
            limit: 100,
        },
    ));
    synapse_server::app(deps)
}

async fn run_as(app: axum::Router, ip: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/api/run")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(r#"{"language":"python","source":"print(1)"}"#))
        .unwrap();
    let res = app.oneshot(request).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn over_the_anonymous_budget_is_a_429_with_the_sign_in_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tiny_budget_app(tmp.path());

    // The first two consume the budget (the refusing executor answers 503 — the GATE runs
    // first, so the meter ticks regardless of the backend).
    for _ in 0..2 {
        let (status, _) = run_as(app.clone(), "203.0.113.7").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }
    let (status, json) = run_as(app.clone(), "203.0.113.7").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json["error"], "Rate limit exceeded");
    assert!(json["detail"].as_str().unwrap().starts_with("Retry after "));
    assert_eq!(json["hint"], "Sign in for a bigger run budget.");

    // A different IP is a different ledger key.
    let (status, _) = run_as(app, "198.51.100.4").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn submissions_share_the_gate_with_their_own_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tiny_budget_app(tmp.path());

    let submit = || {
        Request::builder()
            .method("POST")
            .uri("/api/submissions")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "203.0.113.7")
            .body(Body::from(
                r#"{"path":["nowhere"],"language":"python","source":"x"}"#,
            ))
            .unwrap()
    };
    // Two consumes (404 — no such problem — but the gate ran), then the throttle.
    for _ in 0..2 {
        let res = app.clone().oneshot(submit()).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
    let res = app.oneshot(submit()).await.unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["hint"], "Sign in for a bigger submission budget.");
}
