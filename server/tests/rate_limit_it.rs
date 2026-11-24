//! Integration: the run/submit budget through the REAL router — the 429 envelope, per-IP
//! keying via X-Forwarded-For, the sign-in hint, the shared anonymous ceiling, and admission to
//! the sandbox (the unit windows and limits live beside `rate_limiter` and `admission`).
//!
//! A run needs an account, so the ANONYMOUS ledger is driven through submissions (an open
//! instance takes anonymous submits, and the gate meters before the 404 for an unknown problem),
//! and admission through signed-in runs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use std::sync::Arc;

use synapse_server::platform::admission::Admission;
use synapse_server::platform::rate_limiter::{RateLimitBucket, RateLimiter};
use tower::ServiceExt;

const DEAD_ISSUER: &str = "http://127.0.0.1:9/realms/synapse";

fn bucket(limit: u32) -> RateLimitBucket {
    RateLimitBucket {
        window_seconds: 3600,
        limit,
    }
}

/// The full app with the given (anonymous, signed-in, shared-anonymous) limits, trusting `issuer`.
fn budget_app(root: &std::path::Path, issuer: &str, limits: (u32, u32, u32)) -> axum::Router {
    let mut deps = common::deps_with(root, "http://127.0.0.1:9", None, issuer);
    deps.limiter = Arc::new(RateLimiter::new(
        bucket(limits.0),
        bucket(limits.1),
        bucket(limits.2),
    ));
    synapse_server::app(deps)
}

fn tiny_budget_app(root: &std::path::Path) -> axum::Router {
    budget_app(root, DEAD_ISSUER, (2, 100, 100))
}

async fn send(app: axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let res = app.oneshot(request).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// An anonymous submit of an unknown problem: 404 once metered, 429 once over the budget.
fn submit_from(forwarded_for: &str, cf_connecting_ip: Option<&str>) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/submissions")
        .header("content-type", "application/json")
        .header("x-forwarded-for", forwarded_for);
    if let Some(client) = cf_connecting_ip {
        request = request.header("cf-connecting-ip", client);
    }
    request
        .body(Body::from(
            r#"{"path":["nowhere"],"language":"python","source":"x"}"#,
        ))
        .unwrap()
}

async fn submit_as(app: axum::Router, ip: &str) -> (StatusCode, Value) {
    send(app, submit_from(ip, None)).await
}

fn run_request(bearer: Option<&str>) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/run")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.7");
    if let Some(token) = bearer {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    request
        .body(Body::from(r#"{"language":"python","source":"print(1)"}"#))
        .unwrap()
}

#[tokio::test]
async fn over_the_anonymous_budget_is_a_429_with_the_sign_in_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tiny_budget_app(tmp.path());

    // Two consumes (404 — no such problem — but the gate ran), then the throttle.
    for _ in 0..2 {
        let (status, _) = submit_as(app.clone(), "203.0.113.7").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, json) = submit_as(app.clone(), "203.0.113.7").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json["error"], "Rate limit exceeded");
    assert!(json["detail"].as_str().unwrap().starts_with("Retry after "));
    assert_eq!(json["hint"], "Sign in for a bigger submission budget.");

    // A different IP is a different ledger key.
    let (status, _) = submit_as(app, "198.51.100.4").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_anonymous_run_is_refused_before_it_spends_any_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tiny_budget_app(tmp.path());
    for _ in 0..5 {
        let (status, json) = send(app.clone(), run_request(None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"], "Sign in to run code");
    }
    // The same address still has its whole anonymous budget.
    for _ in 0..2 {
        let (status, _) = submit_as(app.clone(), "203.0.113.7").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_signed_in_run_meters_per_account() {
    let tmp = tempfile::tempdir().unwrap();
    let issuer = common::stub_realm().await;
    let app = budget_app(tmp.path(), &issuer, (100, 2, 100));
    let tester = common::mint(&issuer, "tester");
    for _ in 0..2 {
        let (status, _) = send(app.clone(), run_request(Some(&tester))).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "admitted; the executor refused"
        );
    }
    let (status, json) = send(app.clone(), run_request(Some(&tester))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json["hint"], "Wait for the window to reset.");

    // Another account from the same address keeps its own budget.
    let ada = common::mint(&issuer, "ada");
    let (status, _) = send(app, run_request(Some(&ada))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

// ── the shared anonymous ceiling ──

#[tokio::test]
async fn past_the_anonymous_ceiling_a_fresh_address_is_refused_and_told_why() {
    let tmp = tempfile::tempdir().unwrap();
    let app = budget_app(tmp.path(), DEAD_ISSUER, (100, 100, 2));
    for ip in ["203.0.113.1", "203.0.113.2"] {
        let (status, _) = submit_as(app.clone(), ip).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "metered; no such problem");
    }
    let (status, json) = submit_as(app, "203.0.113.3").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json["error"], "Too many anonymous requests right now");
    assert_eq!(json["hint"], "Sign in to use your own budget.");
}

#[tokio::test]
async fn a_forwarded_for_the_client_wrote_does_not_pick_the_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tiny_budget_app(tmp.path());
    // Every request names a fresh first hop; the edge's appended hop is the same caller.
    for n in 0..2 {
        let (status, _) = submit_as(app.clone(), &format!("10.9.9.{n}, 203.0.113.7")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, _) = submit_as(app, "10.9.9.99, 203.0.113.7").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn an_ipv6_host_is_one_caller_across_its_slash_64() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tiny_budget_app(tmp.path());
    for host in ["2001:db8:1:2::1", "2001:db8:1:2::2"] {
        let (status, _) = submit_as(app.clone(), host).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, _) = submit_as(app, "2001:db8:1:2::abcd").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

// ── admission to the sandbox ──

async fn gated_app(root: &std::path::Path, admission: &Arc<Admission>) -> (axum::Router, String) {
    let issuer = common::stub_realm().await;
    let mut deps = common::deps_with(root, "http://127.0.0.1:9", None, &issuer);
    deps.admission = Arc::clone(admission);
    (synapse_server::app(deps), issuer)
}

#[tokio::test]
async fn a_caller_with_a_run_in_flight_is_refused_until_it_finishes() {
    let tmp = tempfile::tempdir().unwrap();
    let admission = Arc::new(Admission::new(1, 10));
    // A run of this caller's is already in the sandbox (`mint` names the subject `sub-<user>`).
    let held = admission.admit("auth:sub-tester").unwrap();
    let (app, issuer) = gated_app(tmp.path(), &admission).await;
    let tester = common::mint(&issuer, "tester");

    let (status, json) = send(app.clone(), run_request(Some(&tester))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json["error"], "Too many runs in flight");

    // Someone else is admitted, and the place comes back when the run ends.
    let ada = common::mint(&issuer, "ada");
    let (status, _) = send(app.clone(), run_request(Some(&ada))).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "admitted; the executor refused"
    );
    assert_eq!(admission.in_flight(), 1, "the finished run gave its place back");
    drop(held);
    let (status, _) = send(app, run_request(Some(&tester))).await;
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
    let (app, issuer) = gated_app(tmp.path(), &admission).await;
    let tester = common::mint(&issuer, "tester");
    let (status, json) = send(app, run_request(Some(&tester))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json["error"], "The sandbox is busy");
    assert_eq!(json["detail"], "Retry in a few seconds.");
}

// ── behind Cloudflare ──

async fn submit_via_cloudflare(app: axum::Router, edge: &str, client: &str) -> StatusCode {
    send(app, submit_from(edge, Some(client))).await.0
}

#[tokio::test]
async fn one_reader_through_many_cloudflare_edges_is_one_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tiny_budget_app(tmp.path());
    // Cloudflare connects from a different edge from request to request.
    for edge in ["172.70.1.2", "162.158.9.9"] {
        let status = submit_via_cloudflare(app.clone(), edge, "203.0.113.7").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "metered; no such problem");
    }
    let status = submit_via_cloudflare(app.clone(), "104.23.1.1", "203.0.113.7").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    // Someone else behind the same edges keeps their own budget.
    let status = submit_via_cloudflare(app, "172.70.1.2", "198.51.100.4").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_forged_cloudflare_header_from_outside_cloudflare_is_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tiny_budget_app(tmp.path());
    // Straight at the origin, naming a fresh "client" every time: still one caller.
    for n in 0..2 {
        let status = submit_via_cloudflare(app.clone(), "198.51.100.4", &format!("10.0.0.{n}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let status = submit_via_cloudflare(app, "198.51.100.4", "10.0.0.9").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}
