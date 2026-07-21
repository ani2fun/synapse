//! `/robots.txt` + `/sitemap.xml`.
//!
//! Why they live here, in the axum process, rather than behind the Astro sidecar: both are
//! generated from the in-memory catalog index, and the catalog lives HERE. Asking the sidecar
//! to render a sitemap would mean shipping the catalog across a process boundary to format
//! 237 `<loc>` lines. Crawler plumbing mounts UNCONDITIONALLY in `app()`, before the page proxy
//! (`astro_proxy`, mounted when `SYNAPSE_ASTRO_URL` is set), so it never changes identity
//! depending on whether a page front end is configured.

use std::fmt::Write as _;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::catalog::http::LiveCatalogService;

/// Generated, cheap to rebuild, and crawlers re-fetch on their own schedule.
const SITEMAP_CACHE: &str = "public, max-age=3600";

#[derive(Clone)]
pub struct SeoRoutesState {
    pub catalog: Arc<LiveCatalogService>,
    /// Absolute origin for `<loc>` and the `Sitemap:` line — configured, never sniffed from
    /// `Host` (caller-controlled).
    pub site_url: String,
}

pub fn routes(state: SeoRoutesState) -> Router {
    Router::new()
        .route("/robots.txt", get(robots))
        .route("/sitemap.xml", get(sitemap))
        .with_state(state)
}

/// Everything is crawlable except the API and the authenticated surfaces, which have nothing
/// to index and would only burn crawl budget.
async fn robots(State(state): State<SeoRoutesState>) -> Response {
    let body = format!(
        "User-agent: *\n\
         Allow: /\n\
         Disallow: /api/\n\
         Disallow: /account\n\
         Disallow: /admin\n\
         Disallow: /c4/\n\
         \n\
         Sitemap: {}/sitemap.xml\n",
        state.site_url
    );
    respond(body, "text/plain; charset=utf-8")
}

async fn sitemap(State(state): State<SeoRoutesState>) -> Response {
    let Ok(paths) = state.catalog.all_lesson_paths().await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let mut body = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    let origin = &state.site_url;
    let _ = writeln!(body, "  <url><loc>{origin}/</loc></url>");
    let _ = writeln!(body, "  <url><loc>{origin}/blog</loc></url>");
    for path in paths {
        let _ = writeln!(
            body,
            "  <url><loc>{origin}/synapse/{}</loc></url>",
            escape_xml(&path)
        );
    }
    body.push_str("</urlset>\n");
    respond(body, "application/xml; charset=utf-8")
}

/// Owned here rather than shared from elsewhere, so the sitemap's XML escaping has no
/// dependency on any other module's lifetime.
fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn respond(body: String, content_type: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
        .header(header::CACHE_CONTROL, HeaderValue::from_static(SITEMAP_CACHE))
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
