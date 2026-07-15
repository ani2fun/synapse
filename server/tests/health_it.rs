//! Integration test: the walking skeleton's endpoint, driven through the REAL assembled router
//! (`synapse_server::app()`) — middleware and all. What this suite exercises is what the binary
//! serves.

// Test code asserts hard — the banned-in-production panics are the point here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

#[tokio::test]
async fn get_health_returns_the_typed_ok() {
    let app = synapse_server::app();

    let res = app
        .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );

    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json, serde_json::json!({ "status": "ok (walking skeleton)" }));
}

#[tokio::test]
async fn unknown_route_is_a_404() {
    let app = synapse_server::app();

    let res = app
        .oneshot(Request::builder().uri("/api/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
