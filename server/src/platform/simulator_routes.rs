//! Simulator bundles: `GET /simulators/{*rest}` serves each mounted source's `_simulators/`
//! tree — self-contained static apps (`index.html` + relative `./assets/…`) that lessons embed
//! as same-origin iframes. Probed in mount order, first hit wins (the catalog's own rule for
//! duplicate slugs), traversal-guarded per root, explicit content types (the global `nosniff`
//! makes a wrong type fatal for module scripts and stylesheets). A directory request resolves
//! to its `index.html`; a directory WITHOUT the trailing slash 301s to it, because the bundle's
//! relative asset URLs must resolve against the directory. HTML gets one minute of cache (the
//! un-hashed entry point authors replace in place); everything else the same shared hour as
//! `/media` (bundler asset names are content-hashed anyway). No Range support — bundles carry
//! no video. Bare `/simulators` (no id) never matches this route and falls through to the page
//! tier's 404 — deliberate, nothing lives there.

use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::catalog::infrastructure::MountedSources;

const HTML_CACHE: &str = "public, max-age=60";
const ASSET_CACHE: &str = "public, max-age=3600";

/// Every mounted checkout's `_simulators/` tree, probed in mount order.
///
/// Holds the LIVE mounted set (republished by the sync loop), so a satellite registered at
/// runtime serves its simulators without a redeploy — there is no single-checkout constructor,
/// because a frozen boot-time set is exactly the bug that keeps satellite content unserved.
#[derive(Clone)]
pub struct SimulatorRoutes {
    sources: MountedSources,
}

impl SimulatorRoutes {
    pub fn mounted(sources: MountedSources) -> Self {
        Self { sources }
    }

    pub fn routes(&self) -> Router {
        Router::new()
            .route("/simulators/{*rest}", get(simulator))
            .with_state(self.sources.clone())
    }
}

/// What a probe of one root yields: a servable file, or the redirect that teaches the browser
/// the directory form. Typed because the handler branches on it (ADR-RS001).
enum Resolved {
    File(Vec<u8>, &'static str),
    TrailingSlashRedirect,
}

async fn simulator(
    state: axum::extract::State<MountedSources>,
    axum::extract::Path(rest): axum::extract::Path<String>,
) -> Response {
    let roots: Vec<PathBuf> = state
        .0
        .snapshot()
        .into_iter()
        .map(|source| source.root.join("_simulators"))
        .collect();
    let resolved = {
        let rest = rest.clone();
        crate::platform::blocking::run_blocking(move || roots.iter().find_map(|root| resolve_in(root, &rest)))
            .await
    };
    match resolved {
        Some(Resolved::File(bytes, content_type)) => {
            let cache = if content_type.starts_with("text/html") {
                HTML_CACHE
            } else {
                ASSET_CACHE
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
                .header(header::CACHE_CONTROL, HeaderValue::from_static(cache))
                .body(Body::from(bytes))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Some(Resolved::TrailingSlashRedirect) => {
            let location = format!("/simulators/{rest}/");
            match HeaderValue::from_str(&location) {
                Ok(value) => Response::builder()
                    .status(StatusCode::MOVED_PERMANENTLY)
                    .header(header::LOCATION, value)
                    .body(Body::empty())
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
                Err(_) => StatusCode::NOT_FOUND.into_response(),
            }
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Guarded per root, like `/media`: the realpath of the target must stay under the realpath of
/// THAT root, so probing several never widens what any one of them exposes. A root without a
/// `_simulators/` tree fails the first canonicalize and is skipped.
fn resolve_in(root: &Path, rest: &str) -> Option<Resolved> {
    let root_real = root.canonicalize().ok()?;
    let target = root.join(rest.trim_end_matches('/')).canonicalize().ok()?;
    if !target.starts_with(&root_real) {
        return None;
    }
    let file = if target.is_dir() {
        // The browser resolves `./assets/…` against the request path, so the directory form
        // must carry its trailing slash before `index.html` is worth serving.
        if !rest.ends_with('/') {
            return Some(Resolved::TrailingSlashRedirect);
        }
        let index = target.join("index.html");
        if !index.is_file() {
            return None;
        }
        index
    } else if target.is_file() {
        target
    } else {
        return None;
    };
    let content_type = content_type_of(&file);
    std::fs::read(&file)
        .ok()
        .map(|bytes| Resolved::File(bytes, content_type))
}

fn content_type_of(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests;
