//! Edge caching for content GETs. GETs only, 200s only (errors must never be cached), and only
//! the content routes — NEVER `/api/me`, `/api/auth`, `/api/run`, submissions, tutor, health.
//!
//! An ANONYMOUS read is `public, max-age=60, stale-while-revalidate=600` — max-age matches the
//! git-sync cadence, swr keeps far regions warm. A read that carries a BEARER is
//! `private, no-store`: the index and search answer a listed reader with their private books, and
//! a private lesson answers 200, so a signed-in response may hold what another caller must never
//! be handed — and `public` is exactly the directive that lets a shared cache store a response to
//! an authorized request. Both carry `Vary: Authorization`, so the browser never answers a
//! signed-in fetch from the anonymous copy it cached a moment earlier (the reader's private shelf
//! came back empty that way).

use axum::extract::Request;
use axum::http::header::{AUTHORIZATION, CACHE_CONTROL, VARY};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

const PUBLIC: &str = "public, max-age=60, stale-while-revalidate=600";
const PRIVATE: &str = "private, no-store";

fn is_content_path(path: &str) -> bool {
    ["/api/synapse", "/api/blog"]
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

pub async fn stamp(request: Request, next: Next) -> Response {
    let cacheable = request.method() == Method::GET && is_content_path(request.uri().path());
    let signed_in = request.headers().contains_key(AUTHORIZATION);
    let mut response = next.run(request).await;
    if cacheable && response.status() == StatusCode::OK {
        let headers = response.headers_mut();
        let policy = if signed_in { PRIVATE } else { PUBLIC };
        headers.insert(CACHE_CONTROL, HeaderValue::from_static(policy));
        headers.append(VARY, HeaderValue::from_static("authorization"));
    }
    response
}
