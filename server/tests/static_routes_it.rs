//! Integration: the SPA + static assets and the `/c4` proxy (oracle: `StaticRoutesSpec`).
//! The dist is a temp dir; the proxy IT runs a local axum stub upstream — prefix stripping
//! and the 502 degrade are proven against real sockets.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

fn dist(root: &Path) {
    let dist = root.join("dist");
    fs::create_dir_all(dist.join("assets")).unwrap();
    fs::write(dist.join("index.html"), "SPA-INDEX").unwrap();
    fs::write(dist.join("silent-check-sso.html"), "SSO").unwrap();
    fs::write(dist.join("assets/app-abc123.js"), "console.log(1)").unwrap();
    fs::write(root.join("secret.txt"), "no").unwrap();
}

fn app(root: &Path) -> axum::Router {
    let mut deps = common::deps(root);
    deps.static_root = root.join("dist").to_string_lossy().into_owned();
    synapse_server::app(deps)
}

async fn get(app: axum::Router, uri: &str) -> (StatusCode, Option<String>, Option<String>, String) {
    let res = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let read = |name: header::HeaderName| {
        res.headers()
            .get(&name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let cache = read(header::CACHE_CONTROL);
    let content_type = read(header::CONTENT_TYPE);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        cache,
        content_type,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

#[tokio::test]
async fn root_and_index_serve_the_spa_with_no_cache() {
    let tmp = tempfile::tempdir().unwrap();
    dist(tmp.path());
    for uri in ["/", "/index.html"] {
        let (status, cache, _, body) = get(app(tmp.path()), uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(body, "SPA-INDEX");
        assert_eq!(cache.as_deref(), Some("no-cache"), "deploys must show");
    }
}

#[tokio::test]
async fn deep_links_under_reserved_segments_fall_back_to_the_index() {
    let tmp = tempfile::tempdir().unwrap();
    dist(tmp.path());
    for uri in [
        "/synapse/dsa/arrays/two-sum",
        "/blog/some-post",
        "/account",
        "/admin",
    ] {
        let (status, _, _, body) = get(app(tmp.path()), uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(body, "SPA-INDEX", "{uri}");
    }
    // The enumeration never swallows the API: an unknown api path is a real 404.
    let (status, _, _, body) = get(app(tmp.path()), "/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_ne!(body, "SPA-INDEX");
}

#[tokio::test]
async fn hashed_assets_are_immutable_with_their_content_type() {
    let tmp = tempfile::tempdir().unwrap();
    dist(tmp.path());
    let (status, cache, content_type, _) = get(app(tmp.path()), "/assets/app-abc123.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(cache.unwrap().contains("immutable"));
    assert!(content_type.unwrap().contains("javascript"));
}

#[tokio::test]
async fn traversal_cannot_escape_the_static_root() {
    let tmp = tempfile::tempdir().unwrap();
    dist(tmp.path());
    let (status, _, _, body) = get(app(tmp.path()), "/assets/../../secret.txt").await;
    assert_ne!(status, StatusCode::OK);
    assert_ne!(body, "no");
}

#[tokio::test]
async fn an_absent_dist_keeps_the_api_only_root() {
    let tmp = tempfile::tempdir().unwrap();
    // No dist at all — the default deps point at a nonexistent static root.
    let (status, _, _, body) = get(common::app_over(tmp.path()), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("/api/health"), "the dev hint, not the SPA");
}

// ─────────────────────────────────────────────────────────────────────────────
// THE /c4 PROXY
// ─────────────────────────────────────────────────────────────────────────────

/// A stub LikeC4 upstream that echoes what it was asked for.
async fn stub_upstream() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    // The real nginx upstream serves its index at `/`; the wildcard alone would 404 there, so the
    // stub answers both or the trailing-slash test would fail on the STUB rather than the proxy.
    let router = axum::Router::new()
        .route(
            "/",
            axum::routing::get(
                |axum::extract::RawQuery(query): axum::extract::RawQuery| async move {
                    (
                        [(header::CONTENT_TYPE, "text/x-c4")],
                        format!("upstream saw /?{}", query.unwrap_or_default()),
                    )
                },
            ),
        )
        .route(
            "/{*rest}",
            axum::routing::get(
                |axum::extract::Path(rest): axum::extract::Path<String>,
                 axum::extract::RawQuery(query): axum::extract::RawQuery| async move {
                    (
                        [(header::CONTENT_TYPE, "text/x-c4")],
                        format!("upstream saw /{rest}?{}", query.unwrap_or_default()),
                    )
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (addr, handle)
}

#[tokio::test]
async fn the_proxy_strips_the_c4_prefix_and_forwards_the_query() {
    let tmp = tempfile::tempdir().unwrap();
    let (addr, server) = stub_upstream().await;
    let mut deps = common::deps(tmp.path());
    deps.likec4_url = format!("http://{addr}");
    let (status, _, content_type, body) = get(synapse_server::app(deps), "/c4/view/system?theme=dark").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "upstream saw /view/system?theme=dark", "prefix stripped");
    assert_eq!(content_type.as_deref(), Some("text/x-c4"), "content-type copied");
    server.abort();
}

/// Both index forms must serve. `/c4/` needs its own route because axum's `{*rest}` wildcard does
/// not match an empty remainder — it 404'd in production while `/c4` and `/c4/view/…` both worked.
#[tokio::test]
async fn the_proxy_serves_the_index_with_and_without_a_trailing_slash() {
    let tmp = tempfile::tempdir().unwrap();
    let (addr, server) = stub_upstream().await;

    for path in ["/c4", "/c4/"] {
        let mut deps = common::deps(tmp.path());
        deps.likec4_url = format!("http://{addr}");
        let (status, _, _, _) = get(synapse_server::app(deps), path).await;
        assert_eq!(status, StatusCode::OK, "{path} should reach the upstream");
    }
    server.abort();
}

#[tokio::test]
async fn an_unreachable_upstream_is_a_502_never_a_crash() {
    let tmp = tempfile::tempdir().unwrap();
    let (status, _, _, _) = get(common::app_over(tmp.path()), "/c4/view/system").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}
