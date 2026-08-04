//! Integration: `/api/synapse/search` through the REAL stack — router, middleware, filesystem
//! adapter, temp-dir content.
//!
//! The load-bearing case is the one the feature exists for: a term that appears ONLY in a
//! lesson's prose, in a book that arrived from a second source, found with a quote.

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

/// One book whose lesson titles say nothing about what the prose discusses — which is exactly the
/// gap that made this endpoint necessary.
fn seed(root: &Path) {
    write(&root.join("01-learn/category.json"), r#"{"title": "Learn"}"#);
    write(&root.join("01-learn/02-dsa/book.json"), r#"{"title": "DSA"}"#);
    write(
        &root.join("01-learn/02-dsa/01-intro.md"),
        "---\ntitle: Intro\n---\n\nArrays are stored contiguously in memory.\n",
    );
    write(
        &root.join("01-learn/02-dsa/02-lists/01-singly.md"),
        "---\ntitle: Singly\n---\n\nA linked list scatters its nodes across the heap.\n",
    );
}

async fn search(app: axum::Router, query: &str) -> (StatusCode, Option<String>, Value) {
    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/synapse/search?{query}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let cache = res
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        cache,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn a_word_only_the_prose_contains_is_found_with_a_quote() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (status, cache, json) = search(common::app_over(tmp.path()), "q=contiguously").await;

    assert_eq!(status, StatusCode::OK);
    // Proves the route landed under `/api/synapse` and the lesson catch-all did not swallow it.
    assert_eq!(
        cache.as_deref(),
        Some("public, max-age=60, stale-while-revalidate=600")
    );

    let hit = &json["results"][0];
    assert_eq!(hit["title"], "Intro", "no title contains the word");
    assert_eq!(hit["path"], "learn/dsa/intro");
    assert_eq!(hit["breadcrumb"], serde_json::json!(["Learn", "DSA"]));
    assert_eq!(hit["bookSlug"], "dsa");
    assert_eq!(hit["kind"], "lesson");

    let marked: Vec<&str> = hit["snippet"]
        .as_array()
        .expect("a snippet")
        .iter()
        .filter(|segment| segment["marked"] == true)
        .filter_map(|segment| segment["text"].as_str())
        .collect();
    assert_eq!(marked, vec!["contiguously"], "the match comes back marked");
}

#[tokio::test]
async fn the_query_is_echoed_so_a_stale_reply_can_be_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (_, _, json) = search(common::app_over(tmp.path()), "q=scatters").await;
    assert_eq!(json["query"], "scatters");
    assert_eq!(json["results"][0]["title"], "Singly");
}

/// The palette sends whatever has been typed so far, so a half-finished query is not a client
/// error — it is simply not a question yet.
#[tokio::test]
async fn an_empty_query_is_an_empty_answer_not_a_refusal() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let app = common::app_over(tmp.path());
    for query in ["q=", "q=%20%20", "q=!!!"] {
        let (status, _, json) = search(app.clone(), query).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        assert_eq!(json["results"], serde_json::json!([]), "{query}");
    }
}

#[tokio::test]
async fn a_missing_query_parameter_is_still_a_200() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (status, _, json) = search(common::app_over(tmp.path()), "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["results"], serde_json::json!([]));
}

#[tokio::test]
async fn the_limit_is_honoured_and_capped() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let app = common::app_over(tmp.path());

    let (_, _, json) = search(app.clone(), "q=the&limit=1").await;
    assert!(json["results"].as_array().expect("results").len() <= 1);

    // A crafted limit must not turn a cheap read into a large response.
    let (status, _, json) = search(app, "q=the&limit=99999").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["results"].as_array().expect("results").len() <= 50);
}

#[tokio::test]
async fn a_term_in_no_document_answers_empty() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (status, _, json) = search(common::app_over(tmp.path()), "q=zzzznotpresent").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["results"], serde_json::json!([]));
}

/// Freshness rides the same version gate as the catalog: no second cache to go stale on its own.
#[tokio::test]
async fn an_edited_lesson_is_searchable_by_its_new_words() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let app = common::app_over(tmp.path());

    let (_, _, json) = search(app.clone(), "q=quorums").await;
    assert_eq!(json["results"], serde_json::json!([]), "not written yet");

    write(
        &tmp.path().join("01-learn/02-dsa/01-intro.md"),
        "---\ntitle: Intro\n---\n\nNow the prose discusses quorums instead.\n",
    );

    let (_, _, json) = search(app, "q=quorums").await;
    assert_eq!(
        json["results"][0]["title"], "Intro",
        "the index followed the edit"
    );
}
