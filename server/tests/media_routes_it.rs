//! Integration: `/media` — the content checkout's `_media/` tree over the real router:
//! content types, the shared cache hour on BOTH 200 and 206, single-range serving, traversal
//! guard, and origin compression staying off small bodies.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn seed(root: &Path) {
    let media = root.join("_media/dsa");
    fs::create_dir_all(&media).unwrap();
    fs::write(media.join("tree.svg"), "<svg>0123456789</svg>").unwrap();
    fs::write(root.join("secret.md"), "outside the media root").unwrap();
}

#[tokio::test]
async fn media_serves_with_the_shared_cache_hour_and_the_right_type() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    let app = common::app_over(dir.path());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/media/dsa/tree.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/svg+xml"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "public, max-age=3600"
    );
}

#[tokio::test]
async fn a_single_range_answers_206_with_content_range_and_the_same_cache() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    let app = common::app_over(dir.path());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/media/dsa/tree.svg")
                .header(header::RANGE, "bytes=0-4")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers().get(header::CONTENT_RANGE).unwrap(),
        "bytes 0-4/21"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "public, max-age=3600"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<svg>");
}

#[tokio::test]
async fn traversal_out_of_the_media_root_is_a_404() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    let app = common::app_over(dir.path());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/media/dsa/%2E%2E/%2E%2E/secret.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn origin_compression_gzips_big_json_but_leaves_small_bodies_alone() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    // Small (<1 KiB): identity even when gzip is offered.
    let app = common::app_over(dir.path());
    let small = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(small.headers().get(header::CONTENT_ENCODING).is_none());
    // Big (a >1 KiB media file): gzip on the wire.
    let media = dir.path().join("_media/dsa");
    fs::write(media.join("big.svg"), "x".repeat(4096)).unwrap();
    let app = common::app_over(dir.path());
    let big = app
        .oneshot(
            Request::builder()
                .uri("/media/dsa/big.svg")
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(big.headers().get(header::CONTENT_ENCODING).unwrap(), "gzip");
}
