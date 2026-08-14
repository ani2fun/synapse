//! The catalog endpoints. Route shape matters: `/index`, `/search`, `/c4-doc/{id}` and
//! `/d2/{fence}/{file}` are more specific than the `{*paths}` lesson catch-all, and axum's router
//! picks the most specific match. The cost is that a top-level book slugged `search` would be
//! unreachable, exactly as one slugged `index` already is.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use synapse_shared::api::ApiError;
use synapse_shared::catalog::{ComponentDocDto, LessonPayloadDto, SynapseIndexDto};
use synapse_shared::search::SearchResultsDto;

use crate::catalog::application::CatalogService;
use crate::catalog::http::dto;
use crate::catalog::infrastructure::FileSystemContentRepository;
use crate::insights::LessonViewStore;

/// The production service: the catalog over the filesystem adapter (wired in `main`).
pub type LiveCatalogService = CatalogService<FileSystemContentRepository>;

/// The catalog's state. It carries the readership store because serving a lesson is the one
/// place that knows a lesson was read — generic over the port so `catalog/http` depends on
/// `insights`'s CONTRACT, never its Postgres adapter.
pub struct CatalogRoutesState<V> {
    pub service: Arc<LiveCatalogService>,
    pub views: Arc<V>,
}

/// Hand-written: `#[derive(Clone)]` would demand `V: Clone`, which the port does not promise.
impl<V> Clone for CatalogRoutesState<V> {
    fn clone(&self) -> Self {
        Self {
            service: Arc::clone(&self.service),
            views: Arc::clone(&self.views),
        }
    }
}

type CatalogState<V> = State<CatalogRoutesState<V>>;
type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

pub fn routes<V: LessonViewStore + 'static>(state: CatalogRoutesState<V>) -> Router {
    Router::new()
        .route("/api/synapse/index", get(get_synapse_index::<V>))
        .route("/api/synapse/search", get(search_catalog::<V>))
        .route("/api/synapse/c4-doc/{element_id}", get(get_component_doc::<V>))
        .route("/api/synapse/d2/{fence}/{file}", get(get_d2_board::<V>))
        .route("/api/synapse/{*paths}", get(get_synapse_lesson::<V>))
        .with_state(state)
}

/// A board sidecar is name-addressed, like `/media` and for the same reason: authors replace a
/// drawn figure in place rather than minting a new URL for it.
const BOARD_CACHE: &str = "public, max-age=3600";

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
    Query(query): Query<SearchQuery>,
) -> ApiResult<SearchResultsDto> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    match state.service.search(&query.q, limit).await {
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
) -> ApiResult<SynapseIndexDto> {
    tracing::info!("GET /api/synapse/index");
    match state.service.index().await {
        Ok(catalog) => Ok(Json(dto::to_index(&catalog))),
        Err(error) => fail(&error),
    }
}

/// Both sidecar lookups name their lesson the same way: a co-located file is only addressable
/// relative to the lesson that owns it.
#[derive(Deserialize)]
pub struct LessonQuery {
    lesson: String,
}

/// A lesson's directory-mirror path, as the sidecar routes receive it.
fn lesson_segments(lesson: &str) -> Vec<String> {
    lesson
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A LikeC4 component's tutorial doc, looked up next to the given lesson.
#[utoipa::path(
    get,
    path = "/api/synapse/c4-doc/{element_id}",
    operation_id = "getComponentDoc",
    params(
        ("element_id" = String, Path, description = "LikeC4 element id (FQN or leaf)"),
        ("lesson" = String, Query, description = "The lesson's directory-mirror path")
    ),
    responses(
        (status = 200, description = "The component doc", body = ComponentDocDto),
        (status = 404, description = "No such doc", body = ApiError)
    )
)]
pub async fn get_component_doc<V: LessonViewStore>(
    State(state): CatalogState<V>,
    Path(element_id): Path<String>,
    Query(query): Query<LessonQuery>,
) -> ApiResult<ComponentDocDto> {
    tracing::info!(element_id, lesson = query.lesson, "GET /api/synapse/c4-doc");
    let lesson_path = lesson_segments(&query.lesson);
    match state.service.component_doc(&lesson_path, &element_id).await {
        Ok(doc) => Ok(Json(dto::to_component_doc(&doc))),
        Err(error) => fail(&error),
    }
}

/// One board of a `d2 boards` walkthrough — a file from the lesson's `_d2/<fence>/` sidecar.
///
/// Answers the file's own bytes rather than a DTO: these are pre-drawn SVGs and one small
/// manifest, read by `<img>`-free inlining at SSR and by `fetch` when a reader drills down. The
/// hour of cache matches `/media` for the same reason — the path is name-addressed, not
/// content-hashed, so an author replaces a board in place.
#[utoipa::path(
    get,
    path = "/api/synapse/d2/{fence}/{file}",
    operation_id = "getD2Board",
    params(
        ("fence" = String, Path, description = "The walkthrough's name= (its `_d2` directory)"),
        ("file" = String, Path, description = "`<board>.svg` or `boards.json`"),
        ("lesson" = String, Query, description = "The lesson's directory-mirror path")
    ),
    responses(
        (status = 200, description = "The board or its manifest", body = String),
        (status = 404, description = "No such board", body = ApiError)
    )
)]
pub async fn get_d2_board<V: LessonViewStore>(
    State(state): CatalogState<V>,
    Path((fence, file)): Path<(String, String)>,
    Query(query): Query<LessonQuery>,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
    tracing::debug!(fence, file, lesson = query.lesson, "GET /api/synapse/d2");
    let lesson_path = lesson_segments(&query.lesson);
    match state.service.d2_board(&lesson_path, &fence, &file).await {
        Ok(board) => {
            let headers = [
                (header::CONTENT_TYPE, board.file.content_type()),
                (header::CACHE_CONTROL, BOARD_CACHE),
            ];
            Ok((headers, board.body).into_response())
        }
        Err(error) => {
            let (status, body) = dto::to_error(&error);
            Err((status, Json(body)))
        }
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
    match state.service.lesson(&segments).await {
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
