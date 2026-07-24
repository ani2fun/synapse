//! Integration: the content-editing surface (`/api/edits` + `/api/admin/content-editors`) through
//! the REAL assembled router — the whole authoring stack end to end: HTTP → application → the
//! Postgres content-editor allowlist and edit-request store → the dry-run forge → the filesystem
//! lesson source, driven by minted tokens the production verifier accepts.
//!
//! Gated on `POSTGRES_IT` (the stores are real Postgres). The forge is the credential-free dry run
//! `deps_with` wires, so the reuse rule and the whole flow run without GitHub — the GitHub adapter
//! itself is covered separately (`github.rs` wiremock tests).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{gated_pool, mint, stub_realm};
use serde_json::{Value, json};
use sqlx::PgPool;
use tempfile::TempDir;
use tower::ServiceExt;

/// A book with one editable lesson (frontmatter + body). URL path: `book/intro`; the file on disk
/// carries the order prefixes the walker strips, so the IT also proves the URL→file resolution.
fn content() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("01-book");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("book.json"), r#"{"title":"Book"}"#).unwrap();
    fs::write(
        dir.join("01-intro.md"),
        "---\ntitle: Intro\nsummary: The opening.\n---\n\nOriginal prose.\n",
    )
    .unwrap();
    tmp
}

fn app(content_root: &Path, pool: PgPool, issuer: &str) -> Router {
    common::app_with_issuer(content_root, "http://127.0.0.1:9", Some(pool), issuer)
}

async fn call(
    app: Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let res = app.oneshot(request).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 512 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// Remove any rows a prior run left for this test's editor, so counts are exact. Keyed by the
/// unique username, so concurrent IT binaries never collide.
async fn clean(pool: &PgPool, editor: &str) {
    sqlx::query("delete from content_edit_request where username = $1")
        .bind(editor)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("delete from content_editor_allowlist where username = $1")
        .bind(editor)
        .execute(pool)
        .await
        .unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// CONFIG — answers for everyone; canEdit reflects membership
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn config_answers_for_anonymous_and_reflects_membership() {
    let Some(pool) = gated_pool().await else { return };
    let editor = "it-authoring-cfg";
    clean(&pool, editor).await;
    let tmp = content();
    let issuer = stub_realm().await;
    let app = app(tmp.path(), pool.clone(), &issuer);

    // Anonymous: a legitimate answer, not a 401 — the lesson page asks before it knows who reads.
    let (status, body) = call(app.clone(), "GET", "/api/edits/config", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["enabled"], true);
    assert_eq!(body["mode"], "dry-run");
    assert_eq!(body["repo"], "test/content");
    assert_eq!(body["baseBranch"], "main");
    assert_eq!(body["canEdit"], false, "anonymous cannot edit");

    // Signed in but not on the list → canEdit false.
    let token = mint(&issuer, editor);
    let (_, body) = call(app.clone(), "GET", "/api/edits/config", Some(&token), None).await;
    assert_eq!(body["canEdit"], false);

    // Grant them (as the admin tester), and canEdit flips.
    let admin = mint(&issuer, "tester");
    let (status, _) = call(
        app.clone(),
        "POST",
        "/api/admin/content-editors",
        Some(&admin),
        Some(json!({ "username": editor })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = call(app, "GET", "/api/edits/config", Some(&token), None).await;
    assert_eq!(body["canEdit"], true, "granted → may edit");
    clean(&pool, editor).await;
}

// ─────────────────────────────────────────────────────────────────────────────
// THE GATE — anonymous 401, signed-in stranger 403, before any store work
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn source_and_propose_and_list_gate_correctly() {
    let Some(pool) = gated_pool().await else { return };
    let stranger = "it-authoring-stranger";
    clean(&pool, stranger).await;
    let tmp = content();
    let issuer = stub_realm().await;
    let app = app(tmp.path(), pool.clone(), &issuer);

    // Anonymous → 401 on every authed verb.
    for (method, uri, body) in [
        ("GET", "/api/edits/source/book/intro", None),
        ("GET", "/api/edits", None),
        (
            "POST",
            "/api/edits",
            Some(json!({ "lessonPath": "book/intro", "source": "x", "baseFingerprint": "y" })),
        ),
    ] {
        let (status, _) = call(app.clone(), method, uri, None, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }

    // Signed in but not allow-listed → 403 (not 401).
    let token = mint(&issuer, stranger);
    let (status, body) = call(
        app.clone(),
        "GET",
        "/api/edits/source/book/intro",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, _) = call(
        app,
        "POST",
        "/api/edits",
        Some(&token),
        Some(json!({ "lessonPath": "book/intro", "source": "x", "baseFingerprint": "y" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ─────────────────────────────────────────────────────────────────────────────
// THE FLOW — source → propose → reuse → drift → validation → list
// ─────────────────────────────────────────────────────────────────────────────

/// A granted editor with the source already fetched: `(app, token, fingerprint, base source, the
/// content guard)`. The `TempDir` must stay in scope for the content root to exist during the test.
async fn granted(pool: &PgPool, editor: &str) -> (Router, String, String, String, TempDir) {
    clean(pool, editor).await;
    let tmp = content();
    let issuer = stub_realm().await;
    let app = app(tmp.path(), pool.clone(), &issuer);
    let admin = mint(&issuer, "tester");
    call(
        app.clone(),
        "POST",
        "/api/admin/content-editors",
        Some(&admin),
        Some(json!({ "username": editor, "note": "IT" })),
    )
    .await;

    let token = mint(&issuer, editor);
    let (status, src) = call(
        app.clone(),
        "GET",
        "/api/edits/source/book/intro",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{src}");
    assert_eq!(src["lessonPath"], "book/intro");
    assert_eq!(
        src["filePath"], "01-book/01-intro.md",
        "order prefixes preserved on disk"
    );
    assert!(src["source"].as_str().unwrap().starts_with("---\ntitle: Intro"));
    let fingerprint = src["fingerprint"].as_str().unwrap().to_owned();
    let base = src["source"].as_str().unwrap().to_owned();
    (app, token, fingerprint, base, tmp)
}

fn propose(lesson: &str, source: &str, fingerprint: &str) -> Value {
    json!({ "lessonPath": lesson, "source": source, "baseFingerprint": fingerprint })
}

#[tokio::test]
async fn propose_opens_a_branch_then_reuses_it_and_lists_it() {
    let Some(pool) = gated_pool().await else { return };
    let editor = "it-authoring-flow";
    let (app, token, fp, base, _content) = granted(&pool, editor).await;

    // Propose #1 → a fresh branch, one commit, dry-run (no PR).
    let edited = format!("{}\nAn appended sentence.\n", base.trim_end());
    let (status, r1) = call(
        app.clone(),
        "POST",
        "/api/edits",
        Some(&token),
        Some(propose("book/intro", &edited, &fp)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{r1}");
    assert_eq!(r1["branch"], format!("edit/{editor}/book/intro"));
    assert_eq!(r1["state"], "open");
    assert_eq!(r1["commits"], 1);
    assert_eq!(r1["reused"], false);
    assert_eq!(r1["mode"], "dry-run");
    assert!(r1["prNumber"].is_null(), "dry run opens no PR");

    // Propose #2 while it is still open → SAME branch, +1 commit, reused, no second request.
    let edited2 = format!("{}\nAnd a second revision.\n", base.trim_end());
    let (status, r2) = call(
        app.clone(),
        "POST",
        "/api/edits",
        Some(&token),
        Some(propose("book/intro", &edited2, &fp)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{r2}");
    assert_eq!(r2["branch"], r1["branch"]);
    assert_eq!(r2["commits"], 2);
    assert_eq!(r2["reused"], true);

    // My change requests → the one row, two commits.
    let (status, mine) = call(app, "GET", "/api/edits", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let rows = mine.as_array().unwrap();
    assert_eq!(rows.len(), 1, "reuse kept it to one request");
    assert_eq!(rows[0]["commits"], 2);
    clean(&pool, editor).await;
}

#[tokio::test]
async fn propose_rejects_drift_noop_lost_fence_and_unknown_paths() {
    let Some(pool) = gated_pool().await else { return };
    let editor = "it-authoring-reject";
    let (app, token, fp, base, _content) = granted(&pool, editor).await;
    let edited = format!("{}\nchanged\n", base.trim_end());

    // Drift: a stale fingerprint → 409, nothing committed.
    let (status, d) = call(
        app.clone(),
        "POST",
        "/api/edits",
        Some(&token),
        Some(propose("book/intro", &edited, "0000000000000000")),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{d}");

    // No-op (identical) → 400, so no empty PR is ever opened.
    let (status, _) = call(
        app.clone(),
        "POST",
        "/api/edits",
        Some(&token),
        Some(propose("book/intro", &base, &fp)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Frontmatter dropped from a file that had one → 400 (server validate).
    let (status, _) = call(
        app.clone(),
        "POST",
        "/api/edits",
        Some(&token),
        Some(propose("book/intro", "# Intro\n\nno fence\n", &fp)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // A path that is not a lesson → 404.
    let (status, _) = call(app, "GET", "/api/edits/source/book/nope", Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    clean(&pool, editor).await;
}

// ─────────────────────────────────────────────────────────────────────────────
// ADMIN — the content-editor allowlist, gated per call
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn content_editor_admin_gate_and_verbs() {
    let Some(pool) = gated_pool().await else { return };
    let editor = "it-authoring-admin";
    clean(&pool, editor).await;
    let tmp = content();
    let issuer = stub_realm().await;
    let app = app(tmp.path(), pool.clone(), &issuer);

    // Anonymous → 401, a signed-in non-admin → 403.
    let (status, _) = call(app.clone(), "GET", "/api/admin/content-editors", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let stranger = mint(&issuer, "it-authoring-notadmin");
    let (status, body) = call(
        app.clone(),
        "GET",
        "/api/admin/content-editors",
        Some(&stranger),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "Admin only");

    let admin = mint(&issuer, "tester");
    // Grant (canonicalised lowercase), then it appears in the list.
    let (status, granted) = call(
        app.clone(),
        "POST",
        "/api/admin/content-editors",
        Some(&admin),
        Some(json!({ "username": format!("  {}  ", editor.to_uppercase()), "note": "vip" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{granted}");
    assert_eq!(granted["username"], editor, "trimmed + lowercased");

    let (_, list) = call(
        app.clone(),
        "GET",
        "/api/admin/content-editors",
        Some(&admin),
        None,
    )
    .await;
    assert!(
        list.as_array().unwrap().iter().any(|e| e["username"] == editor),
        "granted editor is listed"
    );

    // Blank username → 400.
    let (status, _) = call(
        app.clone(),
        "POST",
        "/api/admin/content-editors",
        Some(&admin),
        Some(json!({ "username": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Revoke → 204, then a second revoke → 404.
    let (status, _) = call(
        app.clone(),
        "DELETE",
        &format!("/api/admin/content-editors/{editor}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(
        app,
        "DELETE",
        &format!("/api/admin/content-editors/{editor}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    clean(&pool, editor).await;
}
