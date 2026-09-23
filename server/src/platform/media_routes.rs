//! Lesson media: `GET /media/{*rest}` serves every mounted checkout's `_media/` tree
//! — traversal-guarded, explicit content types (SVG must be `image/svg+xml`), range-aware
//! (a single `bytes=start[-end]` range → 206), and one shared hour of cache
//! (`public, max-age=3600` on BOTH 200 and 206): media is path-addressed but not
//! content-hashed — authors replace files in place.
//!
//! A PRIVATE source's files are served to its reader list only, the way its prose is: the
//! bearer is verified only when the file turns out to belong to a private source, so public media
//! stays free of the check. An `<img>` carries no bearer, which is why the private lesson island
//! fetches its media itself and hands the browser blob URLs — and why a private file is
//! `no-store`: a shared cache must never hold what the origin only showed to one reader.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::catalog::application::{Audiences, Viewer};
use crate::catalog::infrastructure::MountedSources;
use crate::identity::http::{LiveIdentityService, optional_user};

const MEDIA_CACHE: &str = "public, max-age=3600";
const PRIVATE_MEDIA_CACHE: &str = "private, no-store";

/// Every mounted checkout's `_media/` tree, probed in mount order.
///
/// Collisions cannot arise in practice: the convention is `_media/<book-slug>/…` and book slugs
/// are globally unique, so at most one source can own any given path. Probing in mount order
/// nonetheless matches the catalog's own first-wins rule, so a book being migrated serves its
/// media from the same repository that serves its prose.
#[derive(Clone)]
pub struct MediaRoutes {
    sources: MountedSources,
    audiences: Audiences,
    identity: Arc<LiveIdentityService>,
}

impl MediaRoutes {
    /// Share the live mounted set, so a satellite registered at runtime serves its media too —
    /// and the live audiences, so a source made private stops serving on the same tick.
    pub fn mounted(
        sources: MountedSources,
        audiences: Audiences,
        identity: Arc<LiveIdentityService>,
    ) -> Self {
        Self {
            sources,
            audiences,
            identity,
        }
    }

    pub fn routes(&self) -> Router {
        Router::new()
            .route("/media/{*rest}", get(media))
            .with_state(self.clone())
    }
}

async fn media(
    state: axum::extract::State<MediaRoutes>,
    axum::extract::Path(rest): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Response {
    let roots: Vec<(String, PathBuf)> = state
        .sources
        .snapshot()
        .into_iter()
        .map(|source| (source.id.clone(), source.root.join("_media")))
        .collect();
    let found = tokio::task::spawn_blocking(move || {
        // Guarded per root, exactly as before: the realpath of the target must stay under the
        // realpath of THAT root, so probing several never widens what any one of them exposes.
        roots.iter().find_map(|(source_id, root)| {
            let root_real = root.canonicalize().ok()?;
            let target = root.join(&rest).canonicalize().ok()?;
            if target.starts_with(&root_real) && target.is_file() {
                std::fs::read(&target)
                    .ok()
                    .map(|bytes| (source_id.clone(), bytes, content_type_of(&target)))
            } else {
                None
            }
        })
    })
    .await
    .ok()
    .flatten();
    let Some((source_id, bytes, content_type)) = found else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // The file belongs to a private source: the same gate its prose gets, with the same two
    // answers. Verified only here, so a public file never pays for the check.
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
            return (status, [(header::CACHE_CONTROL, PRIVATE_MEDIA_CACHE)]).into_response();
        }
        PRIVATE_MEDIA_CACHE
    } else {
        MEDIA_CACHE
    };
    let total = bytes.len();
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| parse_range(v, total));
    match range {
        // A single satisfiable range → 206 with Content-Range (video scrubbing et al.).
        Some((start, end)) => {
            let slice = bytes[start..=end].to_vec();
            Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
                .header(header::CACHE_CONTROL, HeaderValue::from_static(cache))
                .header(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"))
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"))
                .body(Body::from(slice))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        None => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
            .header(header::CACHE_CONTROL, HeaderValue::from_static(cache))
            .header(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"))
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
}

/// `bytes=start[-end]` (a single range; suffix/multi ranges fall back to the full 200).
/// Out-of-bounds → `None` (full response): a malformed or unsatisfiable range degrades to the
/// full 200 rather than erroring.
fn parse_range(value: &str, total: usize) -> Option<(usize, usize)> {
    let spec = value.strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let start: usize = start.parse().ok()?;
    let end: usize = if end.is_empty() {
        total.checked_sub(1)?
    } else {
        end.parse().ok()?
    };
    (start <= end && end < total).then_some((start, end))
}

fn content_type_of(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests;
