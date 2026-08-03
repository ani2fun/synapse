//! Integration: `/simulators` — every mounted source's `_simulators/` tree over the real
//! router: index.html resolution, the trailing-slash redirect, the html/asset cache split,
//! explicit bundle content types, the traversal guard, first-source-wins probing, and HEAD
//! (the hydrator's existence probe).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::path::Path;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use synapse_server::catalog::domain::content_tree::PRIMARY_SOURCE_ID;
use synapse_server::catalog::infrastructure::{MountedSources, SourceRoot};
use tower::ServiceExt;

fn seed(root: &Path, body: &str) {
    let sim = root.join("_simulators/osi/assets");
    fs::create_dir_all(&sim).unwrap();
    fs::write(root.join("_simulators/osi/index.html"), body).unwrap();
    fs::write(sim.join("app.js"), "console.log('sim')").unwrap();
    fs::write(sim.join("app.css"), "#root{}").unwrap();
    fs::write(root.join("secret.md"), "outside the simulators root").unwrap();
}

async fn get(app: axum::Router, uri: &str) -> axum::response::Response {
    app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_directory_with_the_trailing_slash_serves_index_html_with_the_short_cache() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), "<html>osi</html>");
    let response = get(common::app_over(dir.path()), "/simulators/osi/").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "public, max-age=60"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html>osi</html>");
}

#[tokio::test]
async fn a_directory_without_the_trailing_slash_redirects_to_it() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), "<html>osi</html>");
    let response = get(common::app_over(dir.path()), "/simulators/osi").await;
    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/simulators/osi/"
    );
}

#[tokio::test]
async fn bundle_assets_carry_their_own_types_and_the_shared_hour() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), "<html>osi</html>");
    let js = get(common::app_over(dir.path()), "/simulators/osi/assets/app.js").await;
    assert_eq!(js.status(), StatusCode::OK);
    assert_eq!(js.headers().get(header::CONTENT_TYPE).unwrap(), "text/javascript");
    assert_eq!(
        js.headers().get(header::CACHE_CONTROL).unwrap(),
        "public, max-age=3600"
    );
    let css = get(common::app_over(dir.path()), "/simulators/osi/assets/app.css").await;
    assert_eq!(
        css.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/css; charset=utf-8"
    );
}

#[tokio::test]
async fn traversal_out_of_the_simulators_root_is_a_404() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), "<html>osi</html>");
    let response = get(
        common::app_over(dir.path()),
        "/simulators/osi/%2E%2E/%2E%2E/secret.md",
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_missing_simulator_is_a_404_not_a_page_fallthrough() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), "<html>osi</html>");
    let response = get(common::app_over(dir.path()), "/simulators/nope/").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn head_answers_200_for_an_existing_bundle() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), "<html>osi</html>");
    let response = common::app_over(dir.path())
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri("/simulators/osi/index.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// The property the live-handle plumbing exists for: a simulator that lives ONLY in a
/// satellite serves, and a duplicated id serves the primary's copy (first-source-wins,
/// the same rule that makes a book cutover safe).
#[tokio::test]
async fn probing_covers_every_mounted_source_primary_first() {
    let primary = tempfile::tempdir().unwrap();
    let satellite = tempfile::tempdir().unwrap();
    seed(primary.path(), "<html>primary copy</html>");
    seed(satellite.path(), "<html>satellite copy</html>");
    let sat_only = satellite.path().join("_simulators/latency");
    fs::create_dir_all(&sat_only).unwrap();
    fs::write(sat_only.join("index.html"), "<html>latency</html>").unwrap();

    let mut deps = common::deps(primary.path());
    deps.mounted = MountedSources::new(vec![
        SourceRoot::new(PRIMARY_SOURCE_ID, primary.path()),
        SourceRoot::new("system-design-guide", satellite.path()),
    ]);
    let app = synapse_server::app(deps);

    let dup = get(app.clone(), "/simulators/osi/").await;
    let body = dup.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html>primary copy</html>");

    let sat = get(app, "/simulators/latency/").await;
    assert_eq!(sat.status(), StatusCode::OK);
    let body = sat.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html>latency</html>");
}
