//! `/robots.txt` + `/sitemap.xml`, extracted from `static_routes.rs` in step A01 of the Astro
//! migration.
//!
//! Why they moved: these two are generated from the in-memory catalog index, and the catalog
//! lives HERE, in the axum process. During the migration the page-serving front end is
//! switchable (`StaticRoutes` today, the Astro sidecar behind `astro_proxy` when
//! `SYNAPSE_ASTRO_URL` is set) — but crawler plumbing must not change identity with the
//! front end, so it mounts UNCONDITIONALLY in `app()`, before either. Post-migration they stay
//! axum-side for the same reason: asking the sidecar to render a sitemap would mean shipping
//! the catalog across a process boundary to format 237 `<loc>` lines.

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

/// Owned here rather than borrowed from `static_routes` — that module's page-serving half is
/// deleted at the end of the migration, and the sitemap must not lose its escape with it.
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
