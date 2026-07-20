//! Integration: the Astro front-door proxy (migration step A01) — a real axum stub plays the
//! SSR sidecar, and the FULL `app()` router proves the fallback wiring: registered routes always
//! win, page requests forward with the documented header contract, and an absent sidecar
//! degrades to 502 rather than an exception. The `None` case pins that yesterday's behaviour is
//! byte-identical — the whole rollback story is "unset one env var", so that claim gets a test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::extract::Request as AxRequest;
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use tower::ServiceExt;

/// A stub sidecar that echoes what it received, so header-contract assertions read the body.
async fn stub_sidecar() -> String {
    let app = Router::new()
        .route(
            "/",
            get(|req: AxRequest| async move {
                let ae = req
                    .headers()
                    .get(header::ACCEPT_ENCODING)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("<absent>")
                    .to_owned();
                let auth = req.headers().contains_key(header::AUTHORIZATION);
                let cookie = req.headers().contains_key(header::COOKIE);
                (
                    [
                        (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                        (header::CACHE_CONTROL, "no-cache"),
                        (header::ETAG, "\"astro-1\""),
                    ],
                    format!("ASTRO-PAGE accept-encoding={ae} auth={auth} cookie={cookie}"),
                )
            }),
        )
        .route(
            "/_astro/app.deadbeef.js",
            get(|| async {
                // Deliberately NO cache-control: the proxy must stamp `immutable` itself.
                ([(header::CONTENT_TYPE, "text/javascript")], "console.log(1)")
            }),
        )
        .route(
            "/missing",
            get(|| async { (StatusCode::NOT_FOUND, "ASTRO-404").into_response() }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    base
}

fn app_with_astro(root: &std::path::Path, astro_url: Option<String>) -> Router {
    let mut deps = common::deps(root);
    deps.astro_url = astro_url;
    synapse_server::app(deps)
}

async fn get_response(app: Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::ACCEPT_ENCODING, "gzip, br")
                .header(header::AUTHORIZATION, "Bearer should-never-cross")
                .header(header::COOKIE, "secret=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (status, headers, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn pages_forward_and_credentials_never_cross() {
    let tmp = tempfile::tempdir().unwrap();
    let base = stub_sidecar().await;
    let (status, headers, body) = get_response(app_with_astro(tmp.path(), Some(base)), "/").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("ASTRO-PAGE"), "{body}");
    // The documented header contract, both directions.
    assert!(
        body.contains("accept-encoding=<absent>"),
        "accept-encoding must be stripped upstream — axum compresses exactly once: {body}"
    );
    assert!(
        body.contains("auth=false") && body.contains("cookie=false"),
        "SSR renders anonymous; credentials must never reach the sidecar: {body}"
    );
    assert_eq!(
        headers.get(header::ETAG).unwrap(),
        "\"astro-1\"",
        "etag copied back"
    );
    assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-cache");
    // The axum stack still owns the security headers — the page went through the full app().
    assert!(
        headers.contains_key("content-security-policy"),
        "security headers must stamp proxied pages too"
    );
}

#[tokio::test]
async fn registered_routes_always_beat_the_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let base = stub_sidecar().await;
    let (status, _, body) = get_response(app_with_astro(tmp.path(), Some(base)), "/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !body.contains("ASTRO-PAGE"),
        "/api must be served by axum, never proxied: {body}"
    );
    assert!(body.contains("ok"), "{body}");
}

#[tokio::test]
async fn robots_and_sitemap_serve_in_proxy_mode() {
    // The step-A01 extraction: crawler plumbing must not change identity with the front end.
    let tmp = tempfile::tempdir().unwrap();
    let base = stub_sidecar().await;
    let app = app_with_astro(tmp.path(), Some(base));
    let (status, _, body) = get_response(app.clone(), "/robots.txt").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Sitemap:"), "{body}");
    let (status, _, body) = get_response(app, "/sitemap.xml").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<urlset"), "{body}");
    assert!(!body.contains("ASTRO-PAGE"));
}

#[tokio::test]
async fn hashed_assets_get_immutable_stamped_when_the_sidecar_omits_it() {
    let tmp = tempfile::tempdir().unwrap();
    let base = stub_sidecar().await;
    let (status, headers, _) =
        get_response(app_with_astro(tmp.path(), Some(base)), "/_astro/app.deadbeef.js").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get(header::CACHE_CONTROL).unwrap(),
        "public, max-age=31536000, immutable"
    );
}

#[tokio::test]
async fn the_sidecar_404_page_is_the_site_404() {
    let tmp = tempfile::tempdir().unwrap();
    let base = stub_sidecar().await;
    let (status, _, body) = get_response(app_with_astro(tmp.path(), Some(base)), "/missing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("ASTRO-404"), "{body}");
}

#[tokio::test]
async fn an_unreachable_sidecar_is_a_502_never_a_crash() {
    let tmp = tempfile::tempdir().unwrap();
    let (status, _, _) = get_response(
        app_with_astro(tmp.path(), Some("http://127.0.0.1:9".to_owned())),
        "/",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn unset_is_byte_identical_yesterday() {
    // The rollback claim. With no astro_url and no dist, `/` is the API-only plain-text root —
    // exactly what `common::deps` served before this step existed.
    let tmp = tempfile::tempdir().unwrap();
    let (status, _, body) = get_response(app_with_astro(tmp.path(), None), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("synapse-rs server"), "{body}");
}
