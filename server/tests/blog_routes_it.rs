//! Integration: the blog endpoints through the REAL stack — router, cache middleware,
//! filesystem adapter, temp-dir posts. Pins the wire shape: `publishedAt` as an ISO string,
//! prev/next slugs, the `ApiError` envelope, and the content-cache stamp on `/api/blog`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::Value;
use tower::ServiceExt;

fn seed(root: &Path) {
    let blog = root.join("blog");
    fs::create_dir_all(&blog).unwrap();
    fs::write(
        blog.join("hello-world.md"),
        "---\ntitle: Hello World\npublishedAt: 2026-06-01\nreadMinutes: 5\n---\n# Hello World\nbody",
    )
    .unwrap();
    fs::write(
        blog.join("older.md"),
        "---\ntitle: Older\npublishedAt: 2026-01-01\n---\nolder body",
    )
    .unwrap();
    fs::write(blog.join("_draft.md"), "# Never ships").unwrap();
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
async fn the_listing_is_newest_first_with_iso_dates_and_the_cache_stamp() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (status, cache, json) = get(common::app_over(tmp.path()), "/api/blog").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        cache.as_deref(),
        Some("public, max-age=60, stale-while-revalidate=600"),
        "/api/blog is a public content read"
    );
    let posts = json.as_array().unwrap();
    assert_eq!(posts.len(), 2, "the draft never ships");
    assert_eq!(posts[0]["slug"], "hello-world");
    assert_eq!(posts[0]["publishedAt"], "2026-06-01");
    assert_eq!(posts[0]["readMinutes"], 5);
    assert_eq!(posts[1]["slug"], "older");
    assert!(posts[0].get("body").is_none(), "summaries carry no body");
}

#[tokio::test]
async fn a_post_carries_body_and_publish_order_neighbours() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (status, _, json) = get(common::app_over(tmp.path()), "/api/blog/hello-world").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["body"], "# Hello World\nbody");
    assert_eq!(json["prev"], "older", "prev = older");
    assert_eq!(json["next"], Value::Null, "nothing newer");
}

#[tokio::test]
async fn an_unknown_slug_is_a_404_api_error() {
    let tmp = tempfile::tempdir().unwrap();
    seed(tmp.path());
    let (status, cache, json) = get(common::app_over(tmp.path()), "/api/blog/no-such-post").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"], "No such post");
    assert_eq!(cache, None, "errors are never cached");
}
