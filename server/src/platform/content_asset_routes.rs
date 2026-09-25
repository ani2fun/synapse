//! Lesson-local assets: `GET /content-assets/{slug path}/_assets/{file}` serves a file from the
//! `_assets/` folder that sits beside a lesson (or at a book's root), so a lesson's widgets live
//! next to the lesson instead of in a repository-wide tree (ADR-RS012).
//!
//! The slug path is the reader URL's (`/synapse/{slug path}`), resolved through the catalog to the
//! lesson file's folder, or to the book's folder when it names a book. Only `_assets/_simulators/`
//! is served: diagram sources and anything else under `_assets/` stay unpublished. A directory
//! resolves to its `index.html`, and a directory without its trailing slash 301s to it, as for
//! `/simulators`.
//!
//! Gated like `/media`: a file of a PRIVATE source goes only to that source's readers (401 to an
//! anonymous caller, 403 to a signed-in one not on the list) and is sent `private, no-store`; the
//! bearer is verified only when the source turns out to be private. An iframe cannot send a
//! bearer, so on a private book the reader island fetches these files itself and hands the frame
//! a self-contained document (`web/src/islands/widgets/Simulator.tsx`).
//!
//! A miss is `no-store`: a CDN that keeps a 404 keeps it for hours, and the file it hides is
//! usually one that was pushed a minute later.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::catalog::application::{Audiences, Viewer};
use crate::catalog::http::routes::LiveCatalogService;
use crate::catalog::infrastructure::MountedSources;
use crate::identity::http::{LiveIdentityService, optional_user};
use crate::platform::simulator_routes::content_type_of;

const PUBLIC_CACHE: &str = "public, max-age=60";
const PRIVATE_CACHE: &str = "private, no-store";
const MISS_CACHE: &str = "no-store";
/// The one `_assets/` subtree this route publishes.
const SERVED: &str = "_simulators";

#[derive(Clone)]
pub struct ContentAssetRoutes {
    catalog: Arc<LiveCatalogService>,
    sources: MountedSources,
    audiences: Audiences,
    identity: Arc<LiveIdentityService>,
}

impl ContentAssetRoutes {
    /// The live catalog (slug path → folder), the live mounted set (source → checkout) and the
    /// live audiences, so a source made private stops serving its assets on the same tick.
    pub fn mounted(
        catalog: Arc<LiveCatalogService>,
        sources: MountedSources,
        audiences: Audiences,
        identity: Arc<LiveIdentityService>,
    ) -> Self {
        Self {
            catalog,
            sources,
            audiences,
            identity,
        }
    }

    pub fn routes(&self) -> Router {
        Router::new()
            .route("/content-assets/{*rest}", get(content_asset))
            .with_state(self.clone())
    }
}

/// `{slug path}/_assets/{file}` → the two halves, or `None` when there is no `_assets` segment or
/// the file is outside the served subtree.
fn split(rest: &str) -> Option<(Vec<String>, &str)> {
    let at = rest.find("/_assets/")?;
    let slugs: Vec<String> = rest[..at].split('/').map(ToOwned::to_owned).collect();
    let file = &rest[at + "/_assets/".len()..];
    let inside = file.strip_prefix(SERVED)?;
    (inside.is_empty() || inside.starts_with('/')).then_some((slugs, file))
}

enum Resolved {
    File(Vec<u8>, &'static str),
    TrailingSlashRedirect,
}

/// `file` (starting `_simulators`) inside `<folder>/_assets/`, guarded: the realpath of the target
/// must stay under the realpath of `<folder>/_assets/_simulators`.
fn resolve_in(assets: &Path, file: &str) -> Option<Resolved> {
    let served_real = assets.join(SERVED).canonicalize().ok()?;
    let target = assets.join(file.trim_end_matches('/')).canonicalize().ok()?;
    if !target.starts_with(&served_real) {
        return None;
    }
    let file_path = if target.is_dir() {
        // Relative script and style URLs resolve against the request path, so the directory
        // form must carry its trailing slash before its index is worth serving.
        if !file.ends_with('/') {
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
    let content_type = content_type_of(&file_path);
    std::fs::read(&file_path)
        .ok()
        .map(|bytes| Resolved::File(bytes, content_type))
}

fn miss() -> Response {
    (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, MISS_CACHE)]).into_response()
}

async fn content_asset(
    state: axum::extract::State<ContentAssetRoutes>,
    axum::extract::Path(rest): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some((slugs, file)) = split(&rest) else {
        return miss();
    };
    let home = match state.catalog.asset_home(&slugs).await {
        Ok(Some(home)) => home,
        Ok(None) => return miss(),
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let (source_id, folder) = home;

    // Gate BEFORE touching the disk, so a private book does not even confirm which files exist.
    let audience = state.audiences.of(&source_id);
    let cache = if audience.is_private() {
        let viewer = match optional_user(&state.identity, &headers).await {
            Ok(Some(user)) => Viewer::User(user.username),
            Ok(None) => Viewer::Anonymous,
            Err((status, body)) => return (status, axum::Json(body.0)).into_response(),
        };
        if !audience.admits(&viewer) {
            let status = if matches!(viewer, Viewer::Anonymous) {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::FORBIDDEN
            };
            return (status, [(header::CACHE_CONTROL, PRIVATE_CACHE)]).into_response();
        }
        PRIVATE_CACHE
    } else {
        PUBLIC_CACHE
    };

    let Some(root) = state
        .sources
        .snapshot()
        .into_iter()
        .find(|source| source.id == source_id)
        .map(|source| source.root)
    else {
        return miss();
    };
    let assets: PathBuf = root.join(&folder).join("_assets");
    let file = file.to_owned();
    let resolved = crate::platform::blocking::run_blocking(move || resolve_in(&assets, &file)).await;
    match resolved {
        Some(Resolved::File(bytes, content_type)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
            .header(header::CACHE_CONTROL, HeaderValue::from_static(cache))
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Some(Resolved::TrailingSlashRedirect) => {
            match HeaderValue::from_str(&format!("/content-assets/{rest}/")) {
                Ok(location) => Response::builder()
                    .status(StatusCode::MOVED_PERMANENTLY)
                    .header(header::LOCATION, location)
                    .header(header::CACHE_CONTROL, HeaderValue::from_static(cache))
                    .body(Body::empty())
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
                Err(_) => miss(),
            }
        }
        None => miss(),
    }
}

#[cfg(test)]
mod tests;
