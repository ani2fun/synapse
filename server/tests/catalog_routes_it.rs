//! Integration: the catalog endpoints through the REAL stack — router, middleware, filesystem
//! adapter, temp-dir content (oracle: `CatalogRoutesSpec`). These tests pin the WIRE SHAPE:
//! `kind` discriminators, full prev/next paths, the `ApiError` envelope, the cache header.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::Value;
use tower::ServiceExt;

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn seed(root: &Path) {
    write(&root.join("01-learn/category.json"), r#"{"title": "Learn"}"#);
    write(&root.join("01-learn/02-dsa/book.json"), r#"{"title": "DSA"}"#);
    write(&root.join("01-learn/02-dsa/01-intro.md"), "# Intro\nwelcome");
    write(
        &root.join("01-learn/02-dsa/02-lists/01-singly.md"),
        "---\ntitle: Singly\nkind: problem\n---\nbody",
    );
    write(
        &root.join("01-learn/02-dsa/02-lists/01-singly.editorial.md"),
        "spoilers",
    );
    write(
        &root.join("01-learn/02-dsa/02-lists/_c4-docs/reader.md"),
        "---\ntitle: Reader\ntechnology: Leptos\n---\nHow it works.",
    );
}

async fn get(app: axum::Router, uri: &str) -> (StatusCode, Option<String>, Value) {
    let res = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let cache = res
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, cache, json)
}

#[tokio::test]
async fn index_returns_the_kind_discriminated_tree_with_the_cache_header() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (status, cache, json) = get(common::app_over(tmp.path()), "/api/synapse/index").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        cache.as_deref(),
        Some("public, max-age=60, stale-while-revalidate=600")
    );

    let category = &json["entries"][0];
    assert_eq!(category["kind"], "category");
    assert_eq!(category["slug"], "learn");
    let book = &category["entries"][0];
    assert_eq!(book["kind"], "book");
    assert_eq!(book["title"], "DSA");
    assert_eq!(book["categoryPath"], serde_json::json!(["learn"]));
    assert_eq!(book["entries"][0]["kind"], "lesson");
    assert_eq!(book["entries"][1]["kind"], "chapter");

    // The index tells problems from prose, so the reader can count a chapter's problems without
    // fetching all of them. `lessonKind` and not `kind`: the enum tag owns that key, and the
    // second assertion is what proves it survived. Prose pays nothing — absent, not null.
    let problem = &book["entries"][1]["entries"][0];
    assert_eq!(problem["lessonKind"], "problem");
    assert_eq!(problem["kind"], "lesson");
    assert!(book["entries"][0].get("lessonKind").is_none());
}

#[tokio::test]
async fn lesson_payload_carries_full_prev_next_and_editorial() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (status, cache, json) = get(
        common::app_over(tmp.path()),
        "/api/synapse/learn/dsa/lists/singly",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        cache.as_deref(),
        Some("public, max-age=60, stale-while-revalidate=600")
    );
    assert_eq!(json["book"]["slug"], "dsa");
    assert_eq!(json["lesson"]["slug"], "singly");
    assert_eq!(json["frontmatter"]["kind"], "problem");
    assert_eq!(json["raw"], "body");
    assert_eq!(json["prev"], "learn/dsa/intro");
    assert_eq!(json["next"], Value::Null);
    assert_eq!(json["editorial"], "spoilers");
}

#[tokio::test]
async fn missing_lessons_404_with_the_api_error_envelope_uncached() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (status, cache, json) = get(common::app_over(tmp.path()), "/api/synapse/learn/nope").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(cache, None, "errors must never carry the cache header");
    assert_eq!(json["error"], "Not found");
    assert!(json["detail"].is_string());
}

#[tokio::test]
async fn component_docs_resolve_dotted_fqns_past_the_lesson_catch_all() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let app = common::app_over(tmp.path());
    let (status, _, json) = get(
        app.clone(),
        "/api/synapse/c4-doc/synapse.client.reader?lesson=learn/dsa/lists/singly",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["technology"], "Leptos");
    assert_eq!(json["body"], "How it works.");

    let (status, _, json) = get(app, "/api/synapse/c4-doc/ghost?lesson=learn/dsa/lists/singly").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"], "Not found");
}

#[tokio::test]
async fn an_invalid_index_is_a_500_api_error() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join("01-dsa/book.json"), "{}");
    write(&tmp.path().join("01-dsa/a.md"), "x");
    write(&tmp.path().join("02-dsa/book.json"), "{}");
    write(&tmp.path().join("02-dsa/a.md"), "x");
    let (status, cache, json) = get(common::app_over(tmp.path()), "/api/synapse/index").await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(cache, None);
    assert_eq!(json["error"], "Catalog index invalid");
}

#[tokio::test]
async fn health_stays_uncached() {
    let tmp = tempfile::tempdir().unwrap();
    let (status, cache, _) = get(common::app_over(tmp.path()), "/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache, None, "only content routes carry the cache header");
}
