//! Integration: `/api/admin/content-sources` and `/api/c4/sources` through the REAL router — the
//! admin gate, the validation that keeps a malformed grouping out of the sitemap, and the
//! unauthenticated C4 list a CI job reads.
//!
//! Over a fake registry (the SQL is the gated Postgres IT) and a local JWKS stub minting real
//! tokens, so what is exercised here is the HTTP layer and nothing beneath it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{mint, stub_realm};
use serde_json::Value;
use synapse_server::catalog::application::{
    ContentSourceDraft, ContentSourceRecord, ContentSources, RegistryError, SyncOutcome, grouping_from_str,
};
use synapse_server::catalog::http::admin::ContentSourceRoutesState;
use tower::ServiceExt;

#[derive(Default)]
struct FakeRegistry {
    rows: Mutex<Vec<ContentSourceRecord>>,
}

impl ContentSources for FakeRegistry {
    async fn list(&self) -> Result<Vec<ContentSourceRecord>, RegistryError> {
        Ok(self.rows.lock().unwrap().clone())
    }
    async fn upsert(&self, draft: &ContentSourceDraft) -> Result<ContentSourceRecord, RegistryError> {
        let id = draft.validate()?;
        let record = ContentSourceRecord {
            id: id.clone(),
            repo: draft.repo.clone(),
            branch: draft.branch.clone(),
            grouping: draft.grouping.clone(),
            order: draft.order,
            enabled: draft.enabled,
            last_sha: None,
            last_synced_at: None,
            last_error: None,
        };
        let mut rows = self.rows.lock().unwrap();
        rows.retain(|r| r.id != id);
        rows.push(record.clone());
        Ok(record)
    }
    async fn remove(&self, id: &str) -> Result<bool, RegistryError> {
        let mut rows = self.rows.lock().unwrap();
        let before = rows.len();
        rows.retain(|r| r.id != id);
        Ok(rows.len() < before)
    }
    async fn record_sync(&self, _id: &str, _outcome: &SyncOutcome) -> Result<(), RegistryError> {
        Ok(())
    }
}

fn seeded(records: Vec<ContentSourceRecord>) -> Arc<FakeRegistry> {
    Arc::new(FakeRegistry {
        rows: Mutex::new(records),
    })
}

fn record(id: &str, grouping: &str, enabled: bool) -> ContentSourceRecord {
    ContentSourceRecord {
        id: id.to_owned(),
        repo: format!("ani2fun/{id}"),
        branch: "main".to_owned(),
        grouping: grouping_from_str(grouping),
        order: Some(7),
        enabled,
        last_sha: Some("abc123".to_owned()),
        last_synced_at: None,
        last_error: None,
    }
}

/// The two routers the app mounts for the registry, over the same state.
fn app(issuer: &str, registry: Arc<FakeRegistry>) -> Router {
    let state = ContentSourceRoutesState {
        sources: registry,
        identity: common::identity_for(issuer),
        admin_users: Arc::new(std::collections::HashSet::from(["tester".to_owned()])),
    };
    synapse_server::catalog::http::admin::routes(state.clone())
        .merge(synapse_server::catalog::http::c4::routes(state))
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

// ── the gate ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_registry_is_admin_only() {
    let issuer = stub_realm().await;
    let registry = seeded(Vec::new());

    // The POST body must be VALID here. Axum runs the `Json` extractor before the handler body,
    // so a malformed one is a 422 before the gate is ever consulted — the same shape the submit
    // and content-editor allowlists have. What this asserts is that a well-formed anonymous
    // request is refused, which is the part that matters.
    let valid = r#"{"repo":"ani2fun/java-guide"}"#;
    for (method, uri) in [
        ("GET", "/api/admin/content-sources"),
        ("POST", "/api/admin/content-sources"),
        ("DELETE", "/api/admin/content-sources/java-guide"),
    ] {
        let (status, _) = call(
            app(&issuer, Arc::clone(&registry)),
            method,
            uri,
            None,
            Some(valid),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
    assert!(
        registry.list().await.unwrap().is_empty(),
        "a refused registration must not have been stored"
    );

    // A verified NON-admin is 403, not 401 — the caller is known, just not permitted.
    let (status, _) = call(
        app(&issuer, registry),
        "GET",
        "/api/admin/content-sources",
        Some(&mint(&issuer, "someone-else")),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ── registering ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn registering_derives_the_id_and_defaults_the_branch() {
    let issuer = stub_realm().await;
    let (status, body) = call(
        app(&issuer, seeded(Vec::new())),
        "POST",
        "/api/admin/content-sources",
        Some(&mint(&issuer, "tester")),
        Some(r#"{"repo":"ani2fun/java-guide","grouping":"programming-languages","order":7}"#),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "java-guide", "derived from the repository name");
    assert_eq!(body["branch"], "main", "defaulted");
    assert_eq!(body["grouping"], "programming-languages");
    assert_eq!(body["enabled"], true, "registered enabled unless told otherwise");
    assert!(body["lastSha"].is_null(), "nothing has been fetched yet");
}

/// The grouping reaches `<loc>` in the sitemap by way of the book's category path, and nothing
/// downstream slug-checks it. Rejecting at the door is the only place this is caught.
#[tokio::test]
async fn a_malformed_grouping_or_repo_is_refused_with_a_reason() {
    let issuer = stub_realm().await;
    for body in [
        r#"{"repo":"ani2fun/java-guide","grouping":"not a slug"}"#,
        r#"{"repo":"java-guide"}"#,
        r#"{"repo":"a/b/c"}"#,
    ] {
        let (status, answer) = call(
            app(&issuer, seeded(Vec::new())),
            "POST",
            "/api/admin/content-sources",
            Some(&mint(&issuer, "tester")),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            answer["detail"].is_string(),
            "the reason must reach the caller: {answer}"
        );
    }
}

#[tokio::test]
async fn removing_reports_whether_anything_was_there() {
    let issuer = stub_realm().await;
    let registry = seeded(vec![record("java-guide", "programming-languages", true)]);
    let token = mint(&issuer, "tester");

    let (status, _) = call(
        app(&issuer, Arc::clone(&registry)),
        "DELETE",
        "/api/admin/content-sources/java-guide",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = call(
        app(&issuer, registry),
        "DELETE",
        "/api/admin/content-sources/java-guide",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "already gone");
}

#[tokio::test]
async fn listing_reports_the_sync_state_the_panel_shows() {
    let issuer = stub_realm().await;
    let (status, body) = call(
        app(
            &issuer,
            seeded(vec![record("java-guide", "programming-languages", true)]),
        ),
        "GET",
        "/api/admin/content-sources",
        Some(&mint(&issuer, "tester")),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["lastSha"], "abc123");
    assert_eq!(body[0]["repo"], "ani2fun/java-guide");
}

// ── the C4 list ──────────────────────────────────────────────────────────────

/// Unauthenticated by design: the consumer is a CI job with no token, and every repository named
/// is already public.
#[tokio::test]
async fn the_c4_source_list_needs_no_credential_and_omits_disabled_sources() {
    let issuer = stub_realm().await;
    let registry = seeded(vec![
        record("system-design-guide", "", true),
        record("parked-guide", "", false),
    ]);

    let (status, body) = call(app(&issuer, registry), "GET", "/api/c4/sources", None, None).await;

    assert_eq!(status, StatusCode::OK);
    let repos: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["repo"].as_str().unwrap())
        .collect();
    assert_eq!(repos, vec!["ani2fun/system-design-guide"]);
    // Sync state is deliberately absent: the build wants sources, not the library.
    assert!(body[0].get("lastSha").is_none(), "{body}");
}
