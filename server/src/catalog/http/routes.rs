//! The catalog endpoints. Route shape matters: `/index`, `/search` and
//! `/d2/{fence}/{file}` are more specific than the `{*paths}` lesson catch-all, and axum's router
//! picks the most specific match. The cost is that a top-level book slugged `search` would be
//! unreachable, exactly as one slugged `index` already is.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use synapse_shared::api::ApiError;
use synapse_shared::catalog::{LessonPayloadDto, SynapseIndexDto};
use synapse_shared::search::SearchResultsDto;

use crate::catalog::application::{CatalogService, Viewer};
use crate::catalog::http::dto;
use crate::catalog::infrastructure::FileSystemContentRepository;
use crate::identity::http::{LiveIdentityService, bearer, optional_user};
use crate::insights::LessonViewStore;

/// The production service: the catalog over the filesystem adapter (wired in `main`).
pub type LiveCatalogService = CatalogService<FileSystemContentRepository>;

/// The catalog's state. It carries the readership store because serving a lesson is the one
/// place that knows a lesson was read — generic over the port so `catalog/http` depends on
/// `insights`'s CONTRACT, never its Postgres adapter.
pub struct CatalogRoutesState<V> {
    pub service: Arc<LiveCatalogService>,
    pub views: Arc<V>,
    /// For the viewer — consulted only when a private book is in play, or a bearer was sent.
    pub identity: Arc<LiveIdentityService>,
}

/// Hand-written: `#[derive(Clone)]` would demand `V: Clone`, which the port does not promise.
impl<V> Clone for CatalogRoutesState<V> {
    fn clone(&self) -> Self {
        Self {
            service: Arc::clone(&self.service),
            views: Arc::clone(&self.views),
            identity: Arc::clone(&self.identity),
        }
    }
}

/// Who is asking, verified ONLY when a bearer was sent. No bearer is the anonymous public tree,
/// unchanged and free; a bearer that does not verify is 401, never silently anonymous (the rule
/// `optional_user` states once for every context). The index and search resolve the viewer this
/// way; a lesson asks `restricts` first and skips even this when the book is public.
async fn viewer_of<V>(
    state: &CatalogRoutesState<V>,
    headers: &axum::http::HeaderMap,
) -> Result<Viewer, (StatusCode, Json<ApiError>)> {
    if bearer(headers).is_none() {
        return Ok(Viewer::Anonymous);
    }
    Ok(optional_user(&state.identity, headers)
        .await?
        .map_or(Viewer::Anonymous, |user| Viewer::User(user.username)))
}

type CatalogState<V> = State<CatalogRoutesState<V>>;
type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

pub fn routes<V: LessonViewStore + 'static>(state: CatalogRoutesState<V>) -> Router {
    Router::new()
        .route("/api/synapse/index", get(get_synapse_index::<V>))
        .route("/api/synapse/search", get(search_catalog::<V>))
        .route("/api/synapse/{*paths}", get(get_synapse_lesson::<V>))
        .with_state(state)
}

/// How many hits one request may ask for. A palette shows a screenful; the ceiling stops a
/// crafted `limit` turning a cheap read into a large response.
const MAX_LIMIT: usize = 50;
const DEFAULT_LIMIT: usize = 20;

#[derive(Debug, Deserialize)]
pub(crate) struct SearchQuery {
    #[serde(default)]
    q: String,
    limit: Option<usize>,
}

/// Full-text search across every mounted source.
///
/// An empty or unusable query is 200 with no results rather than 400: the palette sends whatever
/// has been typed so far, and a half-finished word is not a client error.
#[utoipa::path(
    get,
    path = "/api/synapse/search",
    operation_id = "searchCatalog",
    params(
        ("q" = String, Query, description = "The search query"),
        ("limit" = Option<usize>, Query, description = "Maximum hits (default 20, capped at 50)")
    ),
    responses(
        (status = 200, description = "Ranked hits, best first", body = SearchResultsDto),
        (status = 500, description = "The catalog could not be read", body = ApiError)
    )
)]
pub(crate) async fn search_catalog<V: LessonViewStore>(
    State(state): CatalogState<V>,
    headers: axum::http::HeaderMap,
    Query(query): Query<SearchQuery>,
) -> ApiResult<SearchResultsDto> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let viewer = viewer_of(&state, &headers).await?;
    match state.service.search(&query.q, limit, &viewer).await {
        Ok(hits) => Ok(Json(SearchResultsDto {
            query: query.q,
            results: hits.iter().map(dto::to_search_hit).collect(),
        })),
        Err(error) => fail(&error),
    }
}

fn fail<T>(error: &crate::catalog::application::ContentError) -> ApiResult<T> {
    let (status, body) = dto::to_error(error);
    Err((status, Json(body)))
}

/// The browsable library index.
#[utoipa::path(
    get,
    path = "/api/synapse/index",
    operation_id = "getSynapseIndex",
    responses(
        (status = 200, description = "The catalog", body = SynapseIndexDto),
        (status = 500, description = "Index invalid / IO", body = ApiError)
    )
)]
pub async fn get_synapse_index<V: LessonViewStore>(
    State(state): CatalogState<V>,
    headers: axum::http::HeaderMap,
) -> ApiResult<SynapseIndexDto> {
    tracing::info!("GET /api/synapse/index");
    let viewer = viewer_of(&state, &headers).await?;
    let private = match &viewer {
        // Anonymous never receives a private book, so there is nothing to mark.
        Viewer::Anonymous => Vec::new(),
        Viewer::User(_) => match state.service.private_books().await {
            Ok(slugs) => slugs,
            Err(error) => return fail(&error),
        },
    };
    match state.service.index(&viewer).await {
        Ok(catalog) => Ok(Json(dto::to_index(&catalog, &private))),
        Err(error) => fail(&error),
    }
}

/// A lesson by its full directory-mirror path (the catch-all — registered least specific).
#[utoipa::path(
    get,
    path = "/api/synapse/{paths}",
    operation_id = "getSynapseLesson",
    params(("paths" = String, Path, description = "category…/book/chapter…/lesson")),
    responses(
        (status = 200, description = "The lesson payload", body = LessonPayloadDto),
        (status = 401, description = "The book is private and the caller is anonymous", body = ApiError),
        (status = 403, description = "The book is private and the caller is not on its reader list", body = ApiError),
        (status = 404, description = "No such lesson", body = ApiError)
    )
)]
pub async fn get_synapse_lesson<V: LessonViewStore>(
    State(state): CatalogState<V>,
    headers: axum::http::HeaderMap,
    Path(paths): Path<String>,
) -> ApiResult<LessonPayloadDto> {
    tracing::info!(path = paths, "GET /api/synapse/{{lesson}}");
    let segments: Vec<String> = paths
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    // A public lesson never verifies a token — `record_view` below says why that matters on the
    // read path. Only a private book pays for the check, and only it can refuse.
    let viewer = match state.service.restricts(&segments).await {
        Ok(true) => match optional_user(&state.identity, &headers).await? {
            Some(user) => Viewer::User(user.username),
            None => Viewer::Anonymous,
        },
        Ok(false) => Viewer::Anonymous,
        Err(error) => return fail(&error),
    };
    match state.service.lesson(&segments, &viewer).await {
        Ok(content) => {
            record_view(&state, &segments.join("/"), &headers).await;
            Ok(Json(dto::to_payload(&content)))
        }
        Err(error) => fail(&error),
    }
}

/// Readership, recorded only on a lesson that actually resolved — a 404 is not a read.
///
/// FIRE AND FORGET: a store that is down must never cost the reader their lesson, so the error
/// is logged at `warn` and dropped. The port returns a `Result` precisely so this policy lives
/// here, at the call site, rather than being baked into the store.
///
/// `authed` counts requests that PRESENTED a bearer token, not ones that verified. Verifying
/// would put a JWKS check on the read path of every page view, which is a real cost for one
/// coarse bit — and the bit is only ever read in aggregate.
async fn record_view<V: LessonViewStore>(
    state: &CatalogRoutesState<V>,
    lesson_path: &str,
    headers: &axum::http::HeaderMap,
) {
    let authed = crate::identity::http::bearer(headers).is_some();
    if let Err(error) = state.views.record(lesson_path, authed).await {
        tracing::warn!(lesson_path, %error, "readership not recorded");
    }
}
