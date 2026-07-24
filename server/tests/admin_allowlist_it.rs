//! Integration: `/api/admin/allowlist` — the admin gate and the management verbs through the
//! REAL router, over a fake allowlist (the SQL side is the gated Postgres IT) and a local JWKS
//! stub minting real tokens.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::{TimeZone, Utc};
use common::{mint, stub_realm};
use serde_json::Value;
use synapse_server::submission::application::{AllowlistEntry, SubmissionAllowlist, SubmissionError};
use tower::ServiceExt;

/// A fake allowlist recording grants/revokes, seeded with fixed rows.
#[derive(Default)]
struct FakeAllowlist {
    rows: Mutex<Vec<AllowlistEntry>>,
}

impl FakeAllowlist {
    fn seeded() -> Self {
        let at = |d: u32| Utc.with_ymd_and_hms(2026, 7, d, 0, 0, 0).unwrap();
        Self {
            rows: Mutex::new(vec![
                AllowlistEntry {
                    username: "ada".to_owned(),
                    note: Some("friend".to_owned()),
                    granted_at: at(2),
                },
                AllowlistEntry {
                    username: "zoe".to_owned(),
                    note: None,
                    granted_at: at(1),
                },
            ]),
        }
    }
}

impl SubmissionAllowlist for &'static FakeAllowlist {
    async fn is_allowed(&self, username: &str) -> Result<bool, SubmissionError> {
        Ok(self.rows.lock().unwrap().iter().any(|e| e.username == username))
    }
    async fn list(&self) -> Result<Vec<AllowlistEntry>, SubmissionError> {
        Ok(self.rows.lock().unwrap().clone())
    }
    async fn grant(&self, username: &str, note: Option<&str>) -> Result<AllowlistEntry, SubmissionError> {
        let entry = AllowlistEntry {
            username: username.to_owned(),
            note: note.map(str::to_owned),
            granted_at: Utc.with_ymd_and_hms(2026, 7, 16, 0, 0, 0).unwrap(),
        };
        let mut rows = self.rows.lock().unwrap();
        rows.retain(|e| e.username != username);
        rows.insert(0, entry.clone());
        Ok(entry)
    }
    async fn revoke(&self, username: &str) -> Result<bool, SubmissionError> {
        let mut rows = self.rows.lock().unwrap();
        let before = rows.len();
        rows.retain(|e| e.username != username);
        Ok(rows.len() < before)
    }
}

/// The FULL app over the fake allowlist (`AppDeps` is generic over the port, so this IT
/// doesn't assemble its own sub-router; requests cross the real layer stack).
fn admin_app(issuer: &str, allowlist: &'static FakeAllowlist) -> Router {
    common::app_with_stores(
        issuer,
        Arc::new(allowlist),
        common::lazy_views(),
        common::tutor_off(),
    )
}

async fn call(
    app: Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
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

fn leak_fake(fake: FakeAllowlist) -> &'static FakeAllowlist {
    Box::leak(Box::new(fake))
}

#[tokio::test]
async fn anonymous_is_401_and_a_valid_non_admin_is_403() {
    let issuer = stub_realm().await;
    let app = admin_app(&issuer, leak_fake(FakeAllowlist::seeded()));

    let (status, _) = call(app.clone(), "GET", "/api/admin/allowlist", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // The flag is CONFIG, not a token claim — a perfectly valid stranger is still 403.
    let token = mint(&issuer, "stranger");
    let (status, body) = call(app, "GET", "/api/admin/allowlist", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "Admin only");
}

#[tokio::test]
async fn get_lists_the_grants_for_an_admin() {
    let issuer = stub_realm().await;
    let app = admin_app(&issuer, leak_fake(FakeAllowlist::seeded()));
    let token = mint(&issuer, "tester");
    let (status, body) = call(app, "GET", "/api/admin/allowlist", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = body.as_array().unwrap();
    assert_eq!(rows[0]["username"], "ada");
    assert_eq!(rows[0]["note"], "friend");
    assert_eq!(rows[1]["username"], "zoe");
}

#[tokio::test]
async fn grant_upserts_trimmed_lowercase_and_blank_is_400() {
    let issuer = stub_realm().await;
    let fake = leak_fake(FakeAllowlist::seeded());
    let app = admin_app(&issuer, fake);
    let token = mint(&issuer, "tester");

    let (status, body) = call(
        app.clone(),
        "POST",
        "/api/admin/allowlist",
        Some(&token),
        Some(r#"{"username":"  MixedCase  ","note":"vip"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["username"], "mixedcase", "canonical — matches the verifier");
    assert!(
        fake.rows
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.username == "mixedcase"),
        "stored lowercase"
    );

    let (status, _) = call(
        app,
        "POST",
        "/api/admin/allowlist",
        Some(&token),
        Some(r#"{"username":"   "}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn revoke_is_204_and_unknown_is_404() {
    let issuer = stub_realm().await;
    let fake = leak_fake(FakeAllowlist::seeded());
    let app = admin_app(&issuer, fake);
    let token = mint(&issuer, "tester");

    let (status, _) = call(
        app.clone(),
        "DELETE",
        "/api/admin/allowlist/zoe",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!fake.rows.lock().unwrap().iter().any(|e| e.username == "zoe"));

    let (status, _) = call(app, "DELETE", "/api/admin/allowlist/ghost", Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
