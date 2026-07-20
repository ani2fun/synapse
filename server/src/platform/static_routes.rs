//! The SPA + static assets (oracle: `StaticRoutes`, step 19). The fallback ENUMERATES the
//! SPA's reserved first segments (kept in step with the client's `Page`) instead of a
//! catch-all — a trailing wildcard is greedy enough to shadow `/api` and `/media` (the
//! Cortex-inherited lesson). An absent dist mounts nothing: dev keeps Vite + the API-only
//! root. Hashed assets are immutable for a year; the index is `no-cache` so deploys show.
//!
//! **Step 50 — the head is now per-page.** Until this step `client/index.html` shipped a
//! hardcoded `<title>Synapse</title>` and nothing else, so all 442 lessons were
//! indistinguishable in a search result and every shared link previewed as the same card.
//! Google does execute JavaScript, so the pages *could* be indexed — they just all looked
//! identical, and social crawlers do not run JS at all.
//!
//! The fix is a string substitution, not SSR. Leptos's `ssr` feature would mean hydration and
//! restructuring the client to buy a `<title>` tag. Instead the index is read, its head
//! rewritten from the in-memory catalog index, and served — which the `no-cache` header on the
//! index already made free.

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::catalog::application::PageMeta;
use crate::catalog::http::LiveCatalogService;

/// The client's reserved first segments — `Page::from_segments`'s app-map, mirrored.
const SPA_SEGMENTS: [&str; 4] = ["synapse", "blog", "account", "admin"];

const INDEX_CACHE: &str = "no-cache";
const ASSET_CACHE: &str = "public, max-age=31536000, immutable";

const SITE_NAME: &str = "Synapse";
const DEFAULT_TITLE: &str = "Synapse";
const DEFAULT_DESCRIPTION: &str = "Read, run and understand — interactive lessons on system design, data structures and \
     algorithms, with code you can execute and visualise in the page.";

#[derive(Clone)]
pub struct StaticRoutesState {
    root: PathBuf,
    catalog: Arc<LiveCatalogService>,
    /// Absolute origin for canonical and Open Graph URLs. OG requires absolute URLs, and the
    /// `Host` header is caller-controlled — a configured value cannot be poisoned by a request.
    site_url: String,
}

pub struct StaticRoutes {
    state: StaticRoutesState,
}

impl StaticRoutes {
    pub fn new(static_root: impl AsRef<FsPath>, catalog: Arc<LiveCatalogService>, site_url: &str) -> Self {
        Self {
            state: StaticRoutesState {
                root: static_root.as_ref().to_path_buf(),
                catalog,
                site_url: site_url.trim_end_matches('/').to_owned(),
            },
        }
    }

    /// A production dist is present — dev (no dist) mounts nothing.
    pub fn enabled(&self) -> bool {
        self.state.root.is_dir()
    }

    pub fn routes(&self) -> Router {
        if !self.enabled() {
            return Router::new();
        }
        let mut router = Router::new()
            .route("/", get(index_root))
            .route("/index.html", get(index_root))
            .route("/silent-check-sso.html", get(silent_sso))
            .route("/assets/{*rest}", get(asset));
        for segment in SPA_SEGMENTS {
            router = router.route(&format!("/{segment}"), get(index_root));
            // Only `/synapse/*` resolves to a lesson; the rest share the site-wide head.
            router = if segment == "synapse" {
                router.route("/synapse/{*rest}", get(index_lesson))
            } else {
                router.route(&format!("/{segment}/{{*rest}}"), get(index_root))
            };
        }
        router.with_state(self.state.clone())
    }
}

// ── the head ──────────────────────────────────────────────────────────────────

/// Minimal, and applied to EVERY interpolated value. Titles and summaries are authored content
/// — a stray `"` in a summary would otherwise break out of the `content="…"` attribute it lands
/// in, which is an injection, not a typo.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Rewrite `<title>` and inject the description, canonical and Open Graph tags before `</head>`.
///
/// The `<title>` tag is REPLACED rather than appended to: a second title element is ignored by
/// every consumer, so appending would have looked like it worked.
fn render_head(index_html: &str, title: &str, description: &str, canonical: &str) -> String {
    let title = escape(title);
    let description = escape(description);
    let canonical = escape(canonical);
    let tags = format!(
        "<meta name=\"description\" content=\"{description}\" />\n\
         <link rel=\"canonical\" href=\"{canonical}\" />\n\
         <meta property=\"og:type\" content=\"website\" />\n\
         <meta property=\"og:site_name\" content=\"{SITE_NAME}\" />\n\
         <meta property=\"og:title\" content=\"{title}\" />\n\
         <meta property=\"og:description\" content=\"{description}\" />\n\
         <meta property=\"og:url\" content=\"{canonical}\" />\n\
         <meta name=\"twitter:card\" content=\"summary\" />\n\
         <meta name=\"twitter:title\" content=\"{title}\" />\n\
         <meta name=\"twitter:description\" content=\"{description}\" />\n\
         </head>"
    );
    let with_title = replace_title(index_html, &title);
    with_title.replacen("</head>", &tags, 1)
}

fn replace_title(html: &str, title: &str) -> String {
    const CLOSE: &str = "</title>";
    let Some(open) = html.find("<title>") else {
        return html.to_owned();
    };
    let Some(close) = html[open..].find(CLOSE).map(|i| open + i) else {
        return html.to_owned();
    };
    format!(
        "{}<title>{}{CLOSE}{}",
        &html[..open],
        title,
        &html[close + CLOSE.len()..]
    )
}

/// `Book · Lesson` reads better than the reverse in a tab strip and a search result, where the
/// left of the string is what survives truncation.
fn title_for(meta: &PageMeta) -> String {
    format!("{} · {} — {SITE_NAME}", meta.book_title, meta.title)
}

// ── handlers ──────────────────────────────────────────────────────────────────

async fn index_root(State(state): State<StaticRoutesState>) -> Response {
    serve_index(
        &state,
        DEFAULT_TITLE.to_owned(),
        DEFAULT_DESCRIPTION.to_owned(),
        "/",
    )
    .await
}

async fn index_lesson(State(state): State<StaticRoutesState>, Path(rest): Path<String>) -> Response {
    let segments: Vec<String> = rest
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    // A catalog that cannot answer must not take the page down: the SPA still renders, it just
    // renders under the site-wide head. Same degradation as an unknown route.
    let meta = state.catalog.page_meta(&segments).await.ok().flatten();
    let (title, description) = match &meta {
        Some(meta) => (
            title_for(meta),
            meta.description
                .clone()
                .unwrap_or_else(|| DEFAULT_DESCRIPTION.to_owned()),
        ),
        None => (DEFAULT_TITLE.to_owned(), DEFAULT_DESCRIPTION.to_owned()),
    };
    serve_index(&state, title, description, &format!("/synapse/{rest}")).await
}

async fn serve_index(state: &StaticRoutesState, title: String, description: String, path: &str) -> Response {
    let Some(bytes) = read_under(&state.root, "index.html").await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(html) = String::from_utf8(bytes) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let canonical = format!("{}{}", state.site_url, path);
    let rendered = render_head(&html, &title, &description, &canonical);
    html_response(rendered.into_bytes(), INDEX_CACHE)
}

async fn silent_sso(State(state): State<StaticRoutesState>) -> Response {
    serve(
        &state.root,
        "silent-check-sso.html",
        "text/html; charset=utf-8",
        INDEX_CACHE,
    )
    .await
}

/// Everything is crawlable except the API and the authenticated surfaces, which have nothing
/// to index and would only burn crawl budget.
async fn asset(State(state): State<StaticRoutesState>, Path(rest): Path<String>) -> Response {
    let rel = format!("assets/{rest}");
    serve(&state.root, &rel, content_type_of(&rel), ASSET_CACHE).await
}

// ── responses ─────────────────────────────────────────────────────────────────

fn html_response(bytes: Vec<u8>, cache: &'static str) -> Response {
    build(bytes, "text/html; charset=utf-8", cache)
}

fn build(bytes: Vec<u8>, content_type: &'static str, cache: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
        .header(header::CACHE_CONTROL, HeaderValue::from_static(cache))
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Traversal-guarded read: the REAL path must stay under the real root.
async fn read_under(root: &FsPath, rel: &str) -> Option<Vec<u8>> {
    let root = root.to_path_buf();
    let rel = rel.to_owned();
    tokio::task::spawn_blocking(move || {
        let root_real = root.canonicalize().ok()?;
        let target = root.join(&rel).canonicalize().ok()?;
        if target.starts_with(&root_real) && target.is_file() {
            std::fs::read(target).ok()
        } else {
            None
        }
    })
    .await
    .ok()
    .flatten()
}

async fn serve(root: &FsPath, rel: &str, content_type: &'static str, cache: &'static str) -> Response {
    match read_under(root, rel).await {
        Some(bytes) => build(bytes, content_type, cache),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn content_type_of(rel: &str) -> &'static str {
    match FsPath::new(rel).extension().and_then(|ext| ext.to_str()) {
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
#[path = "static_routes_tests.rs"]
mod tests;
