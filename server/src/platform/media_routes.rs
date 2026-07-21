//! Lesson media: `GET /media/{*rest}` serves `content_root/_media`
//! — traversal-guarded, explicit content types (SVG must be `image/svg+xml`), range-aware
//! (a single `bytes=start[-end]` range → 206), and one shared hour of cache
//! (`public, max-age=3600` on BOTH 200 and 206): media is path-addressed but not
//! content-hashed — authors replace files in place.

use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

const MEDIA_CACHE: &str = "public, max-age=3600";

pub struct MediaRoutes {
    root: PathBuf,
}

impl MediaRoutes {
    /// The media root is the content checkout's `_media/` tree.
    pub fn new(content_root: impl AsRef<Path>) -> Self {
        Self {
            root: content_root.as_ref().join("_media"),
        }
    }

    pub fn routes(&self) -> Router {
        Router::new()
            .route("/media/{*rest}", get(media))
            .with_state(self.root.clone())
    }
}

async fn media(
    state: axum::extract::State<PathBuf>,
    axum::extract::Path(rest): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Response {
    let root = state.0.clone();
    let bytes = tokio::task::spawn_blocking(move || {
        let root_real = root.canonicalize().ok()?;
        let target = root.join(&rest).canonicalize().ok()?;
        if target.starts_with(&root_real) && target.is_file() {
            std::fs::read(&target)
                .ok()
                .map(|bytes| (bytes, content_type_of(&target)))
        } else {
            None
        }
    })
    .await
    .ok()
    .flatten();
    let Some((bytes, content_type)) = bytes else {
        return StatusCode::NOT_FOUND.into_response();
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
                .header(header::CACHE_CONTROL, HeaderValue::from_static(MEDIA_CACHE))
                .header(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"))
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"))
                .body(Body::from(slice))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        None => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
            .header(header::CACHE_CONTROL, HeaderValue::from_static(MEDIA_CACHE))
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::parse_range;

    #[test]
    fn parses_a_single_bounded_range() {
        assert_eq!(parse_range("bytes=0-4", 10), Some((0, 4)));
        assert_eq!(parse_range("bytes=5-", 10), Some((5, 9)));
    }

    #[test]
    fn out_of_bounds_or_malformed_falls_back_to_the_full_response() {
        assert_eq!(parse_range("bytes=5-20", 10), None);
        assert_eq!(parse_range("bytes=7-5", 10), None);
        assert_eq!(parse_range("bites=0-4", 10), None);
        assert_eq!(parse_range("bytes=0-4", 0), None);
    }
}
