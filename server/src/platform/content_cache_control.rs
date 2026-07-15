//! Edge caching for public content GETs (oracle: `ContentCacheControl`, ADR-S033):
//! `public, max-age=60, stale-while-revalidate=600` — max-age matches the git-sync cadence,
//! swr keeps far regions warm. GETs only, 200s only (errors must never be cached), and only
//! the content routes — NEVER `/api/me`, `/api/auth`, `/api/run`, submissions, tutor, health.

use axum::extract::Request;
use axum::http::header::CACHE_CONTROL;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

const HEADER: &str = "public, max-age=60, stale-while-revalidate=600";

fn is_content_path(path: &str) -> bool {
    ["/api/synapse", "/api/blog"]
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

pub async fn stamp(request: Request, next: Next) -> Response {
    let cacheable = request.method() == Method::GET && is_content_path(request.uri().path());
    let mut response = next.run(request).await;
    if cacheable && response.status() == StatusCode::OK {
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static(HEADER));
    }
    response
}
