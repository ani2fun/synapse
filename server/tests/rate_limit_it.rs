//! Integration: the run/submit budget through the REAL router — the 429 envelope, per-IP
//! keying via X-Forwarded-For, the sign-in hint, the shared anonymous ceiling, and admission to
//! the sandbox (the unit windows and limits live beside `rate_limiter` and `admission`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use std::sync::Arc;

use synapse_server::platform::admission::Admission;
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

// ── the shared anonymous ceiling ──

fn ceiling_app(root: &std::path::Path, ceiling: u32) -> axum::Router {
    let mut deps = common::deps(root);
    let roomy = RateLimitBucket {
        window_seconds: 3600,
        limit: 100,
    };
    deps.limiter = Arc::new(RateLimiter::new(
        roomy,
        roomy,
        RateLimitBucket {
            window_seconds: 3600,
            limit: ceiling,
        },
    ));
    synapse_server::app(deps)
}

#[tokio::test]
async fn past_the_anonymous_ceiling_a_fresh_address_is_refused_and_told_why() {
    let tmp = tempfile::tempdir().unwrap();
    let app = ceiling_app(tmp.path(), 2);
    for ip in ["203.0.113.1", "203.0.113.2"] {
        let (status, _) = run_as(app.clone(), ip).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "admitted; the executor refused"
        );
    }
    let (status, json) = run_as(app, "203.0.113.3").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json["error"], "Too many anonymous runs right now");
    assert_eq!(json["hint"], "Sign in to run on your own budget.");
}

#[tokio::test]
async fn a_forwarded_for_the_client_wrote_does_not_pick_the_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tiny_budget_app(tmp.path());
    // Every request names a fresh first hop; the edge's appended hop is the same caller.
    for n in 0..2 {
        let (status, _) = run_as(app.clone(), &format!("10.9.9.{n}, 203.0.113.7")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }
    let (status, _) = run_as(app, "10.9.9.99, 203.0.113.7").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

// ── admission to the sandbox ──

fn gated_app(root: &std::path::Path, admission: &Arc<Admission>) -> axum::Router {
    let mut deps = common::deps(root);
    deps.admission = Arc::clone(admission);
    synapse_server::app(deps)
}

#[tokio::test]
async fn a_caller_with_a_run_in_flight_is_refused_until_it_finishes() {
    let tmp = tempfile::tempdir().unwrap();
    let admission = Arc::new(Admission::new(1, 10));
    // A run of this caller's is already in the sandbox.
    let held = admission.admit("anon:203.0.113.7").unwrap();
    let app = gated_app(tmp.path(), &admission);

    let (status, json) = run_as(app.clone(), "203.0.113.7").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json["error"], "Too many runs in flight");

    // Someone else is admitted, and the place comes back when the run ends.
    let (status, _) = run_as(app.clone(), "198.51.100.4").await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "admitted; the executor refused"
    );
    assert_eq!(admission.in_flight(), 1, "the finished run gave its place back");
    drop(held);
    let (status, _) = run_as(app, "203.0.113.7").await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "admitted once its run finished"
    );
    assert_eq!(admission.in_flight(), 0);
}

#[tokio::test]
async fn a_full_sandbox_refuses_at_once_with_a_503() {
    let tmp = tempfile::tempdir().unwrap();
    let admission = Arc::new(Admission::new(2, 1));
    let _held = admission.admit("auth:someone").unwrap();
    let (status, json) = run_as(gated_app(tmp.path(), &admission), "203.0.113.7").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json["error"], "The sandbox is busy");
    assert_eq!(json["detail"], "Retry in a few seconds.");
}

#[tokio::test]
async fn an_ipv6_host_is_one_caller_across_its_slash_64() {
    let tmp = tempfile::tempdir().unwrap();
    let admission = Arc::new(Admission::new(1, 10));
    let _held = admission.admit("anon:2001:db8:1:2::/64").unwrap();
    let (status, _) = run_as(gated_app(tmp.path(), &admission), "2001:db8:1:2::abcd").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}
