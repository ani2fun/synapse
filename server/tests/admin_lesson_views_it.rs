//! Integration: `/api/admin/lesson-views` — the admin gate and the read, through the REAL
//! router over a fake store (the SQL side is the gated Postgres IT) and a local JWKS stub
//! minting real tokens. Same shape as `admin_allowlist_it.rs`, which is the reference for
//! driving a port fake through a generic router.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::routing::get;
use chrono::{TimeZone, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{Value, json};
use synapse_server::insights::{InsightsError, LessonViewCount, LessonViewStore};
use tower::ServiceExt;

const TEST_PEM: &str = include_str!("fixtures/test-only-rsa.pem");
const KID: &str = "synapse-test";
const N_B64: &str = "zhViOX4PnOD51OW9MWknnaOwPKP1lodDI-BX4tk4Ulq6yj816CV89b9F-TXuUkHEXToXrheogf8gAIuYpx1PJD-e2spf9mIbKqmMFTSHZv36GIWsZ-afRr9vhSFhRkf8Jix9Yoo8au9JnbhkkkexXWg_j-w-ct5jTXwBBq-Sy72ijxKZ3Hrv0IkKIdYbwbVY57FLd7GM_cJOioCsqZuuw3HscaP33CUIpuXWam-q5tejXFlR7ldo9qrpuuPfcJUwh9Jgz4UA79asREpyyKkOv7IczvXODWYtSQYRK6bLgpuiIvwiDQ8M2K02OH-dYtIJ2euWYH6h2VNqabcZ36zDFw";

/// A fake store recording what was written and returning fixed counts, so the route's ordering,
/// clamping and DTO mapping are pinned without a database.
#[derive(Default)]
struct FakeViews {
    recorded: Mutex<Vec<(String, bool)>>,
    limits: Mutex<Vec<i64>>,
}

impl LessonViewStore for &'static FakeViews {
    async fn record(&self, lesson_path: &str, authed: bool) -> Result<(), InsightsError> {
        self.recorded
            .lock()
            .unwrap()
            .push((lesson_path.to_owned(), authed));
        Ok(())
    }

    async fn top(&self, limit: i64) -> Result<Vec<LessonViewCount>, InsightsError> {
        self.limits.lock().unwrap().push(limit);
        let at = |d: u32| Utc.with_ymd_and_hms(2026, 7, d, 12, 0, 0).unwrap();
        Ok(vec![
            LessonViewCount {
                lesson_path: "learn/dsa/lists/singly".to_owned(),
                views: 42,
                authed_views: 7,
                last_viewed: at(19),
            },
            LessonViewCount {
                lesson_path: "learn/python/intro".to_owned(),
                views: 9,
                authed_views: 0,
                last_viewed: at(18),
            },
        ])
    }
}

async fn stub_realm() -> String {
    let jwks = json!({
        "keys": [{ "kty": "RSA", "alg": "RS256", "use": "sig", "kid": KID, "n": N_B64, "e": "AQAB" }]
    })
    .to_string();
    let app = Router::new().route(
        "/realms/synapse/protocol/openid-connect/certs",
        get(move || {
            let jwks = jwks.clone();
            async move { ([(header::CONTENT_TYPE, "application/json")], jwks) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}/realms/synapse", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    issuer
}

fn mint(issuer: &str, username: &str) -> String {
    let now = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()).unwrap();
    let claims = json!({
        "sub": format!("sub-{username}"),
        "iss": issuer,
        "exp": now + 300,
        "aud": "account",
        "azp": "synapse-web",
        "preferred_username": username,
    });
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(KID.to_owned());
    let key = EncodingKey::from_rsa_pem(TEST_PEM.as_bytes()).unwrap();
    jsonwebtoken::encode(&header, &claims, &key).unwrap()
}

/// The FULL app over the fake store (`AppDeps` is generic over the port, so this IT doesn't
/// assemble its own sub-router; requests cross the real layer stack).
fn views_app(issuer: &str, views: &'static FakeViews) -> Router {
    common::app_with_stores(
        issuer,
        common::lazy_allowlist(),
        Arc::new(views),
        common::tutor_off(),
    )
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Named `req`, not `get`: `axum::routing::get` is already in scope for the JWKS stub.
fn req(uri: &str, token: Option<&str>) -> Request<Body> {
    let builder = Request::builder().uri(uri);
    let builder = match token {
        Some(t) => builder.header(header::AUTHORIZATION, format!("Bearer {t}")),
        None => builder,
    };
    builder.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn anonymous_is_401() {
    static VIEWS: std::sync::OnceLock<FakeViews> = std::sync::OnceLock::new();
    let views = VIEWS.get_or_init(FakeViews::default);
    let issuer = stub_realm().await;
    let response = views_app(&issuer, views)
        .oneshot(req("/api/admin/lesson-views", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(body_json(response).await["error"], "Missing bearer token");
    // The gate runs BEFORE the store — an anonymous call must not even reach it.
    assert!(views.limits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_verified_non_admin_is_403_and_never_reaches_the_store() {
    static VIEWS: std::sync::OnceLock<FakeViews> = std::sync::OnceLock::new();
    let views = VIEWS.get_or_init(FakeViews::default);
    let issuer = stub_realm().await;
    let token = mint(&issuer, "test1");
    let response = views_app(&issuer, views)
        .oneshot(req("/api/admin/lesson-views", Some(&token)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_json(response).await;
    assert_eq!(body["error"], "Admin only");
    assert_eq!(
        body["detail"], "'test1' is not an admin on this deployment",
        "the 403 copy is the oracle's, verbatim"
    );
    assert!(views.limits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_admin_reads_counts_most_read_first() {
    static VIEWS: std::sync::OnceLock<FakeViews> = std::sync::OnceLock::new();
    let views = VIEWS.get_or_init(FakeViews::default);
    let issuer = stub_realm().await;
    let token = mint(&issuer, "tester");
    let response = views_app(&issuer, views)
        .oneshot(req("/api/admin/lesson-views", Some(&token)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body[0]["lessonPath"], "learn/dsa/lists/singly");
    assert_eq!(body[0]["views"], 42);
    assert_eq!(body[0]["authedViews"], 7);
    assert_eq!(
        body[0]["lastViewed"], "2026-07-19T12:00:00Z",
        "ISO-8601 UTC as a string, matching blog's publishedAt convention"
    );
    assert_eq!(body[1]["lessonPath"], "learn/python/intro");
    // No reader ever crosses the wire — the store cannot answer "who", because it never recorded it.
    assert!(body[0].get("userId").is_none());
}

#[tokio::test]
async fn the_limit_is_defaulted_and_clamped() {
    static VIEWS: std::sync::OnceLock<FakeViews> = std::sync::OnceLock::new();
    let views = VIEWS.get_or_init(FakeViews::default);
    let issuer = stub_realm().await;
    let token = mint(&issuer, "tester");
    for uri in [
        "/api/admin/lesson-views",
        "/api/admin/lesson-views?limit=5",
        "/api/admin/lesson-views?limit=99999",
        "/api/admin/lesson-views?limit=0",
        "/api/admin/lesson-views?limit=-3",
    ] {
        let response = views_app(&issuer, views)
            .oneshot(req(uri, Some(&token)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
    }
    assert_eq!(
        *views.limits.lock().unwrap(),
        vec![50, 5, 500, 1, 1],
        "absent → 50; over → 500; zero and negative → 1 (a caller cannot ask for the whole table)"
    );
}
