//! The `/api/synapse/d2` reverse proxy: a walkthrough's boards, from the `d2-render` sidecar.
//!
//! A `d2 boards` fence compiles to a TREE of boards. The page tier asks the renderer for the
//! whole walkthrough while rendering the lesson and inlines the ROOT; the reader fetches the rest
//! through here as they click into them. The renderer drew every board when the root was asked
//! for, so these are cache reads.
//!
//! CONTENT-ADDRESSED: `{hash}` is the fence source's hash, `{slug}` the board's. That is what
//! retired the `_d2/<fence>/` sidecars committed beside a lesson (ADR-RS009) — a walkthrough is
//! identified by what it IS, so the same diagram in two lessons is one set of boards and no
//! `?lesson=` is needed to find them.
//!
//! GET-only, buffered, `content-type` forced to SVG; an unreachable upstream is a 502, never an
//! exception. Mounted only when a renderer is configured, so with none the route is a structural
//! 404 and the reader's viewer compiles the walkthrough itself — the same floor every d2 miss
//! lands on.

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

/// Immutable by construction: the path IS the content. Unlike `/media`, which is path-addressed
/// and replaced in place, a board can be held forever.
const BOARD_CACHE: &str = "public, max-age=31536000, immutable";

#[derive(Clone)]
struct ProxyState {
    client: reqwest::Client,
    upstream_base: String,
}

pub fn routes(upstream_base: &str) -> Router {
    let client = reqwest::Client::builder()
        .http1_only()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        // Builder failure = TLS backend missing at boot — a config bug, not a request error.
        .unwrap_or_default();
    let state = ProxyState {
        client,
        upstream_base: upstream_base.trim_end_matches('/').to_owned(),
    };
    Router::new()
        .route("/api/synapse/d2/{hash}/{slug}", get(proxy))
        .with_state(state)
}

/// A hash and a slug both reach a filesystem path inside the renderer, so they are checked HERE
/// rather than trusted because the manifest that named them was generated. Same alphabet the
/// renderer's own key check uses; anything else is a 404, not a forwarded request.
fn addressable(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('.')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

async fn proxy(State(state): State<ProxyState>, Path((hash, slug)): Path<(String, String)>) -> Response {
    let slug = slug.strip_suffix(".svg").unwrap_or(&slug).to_owned();
    if !addressable(&hash) || !addressable(&slug) {
        return StatusCode::NOT_FOUND.into_response();
    }
    tracing::debug!(hash, slug, "GET /api/synapse/d2");
    let url = format!("{}/board/{hash}/{slug}", state.upstream_base);
    match state.client.get(&url).send().await {
        Ok(upstream) => {
            let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            if !status.is_success() {
                // A board nobody drew is a 404 the viewer handles, not a body worth relaying.
                return status.into_response();
            }
            match upstream.bytes().await {
                Ok(body) => (
                    [
                        (header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml")),
                        (header::CACHE_CONTROL, HeaderValue::from_static(BOARD_CACHE)),
                    ],
                    Body::from(body),
                )
                    .into_response(),
                Err(_) => StatusCode::BAD_GATEWAY.into_response(),
            }
        }
        Err(error) => {
            tracing::warn!(%error, url, "d2-render unreachable");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

#[cfg(test)]
mod tests;
