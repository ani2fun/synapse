//! The three catalog endpoints. Route shape matters: `/index` and `/c4-doc/{id}` are more
//! specific than the `{*paths}` lesson catch-all, and axum's router picks the most specific
//! match.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use synapse_shared::api::ApiError;
use synapse_shared::catalog::{ComponentDocDto, LessonPayloadDto, SynapseIndexDto};

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
        .route("/api/synapse/c4-doc/{element_id}", get(get_component_doc::<V>))
        .route("/api/synapse/{*paths}", get(get_synapse_lesson::<V>))
        .with_state(state)
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

#[derive(Deserialize)]
pub struct C4DocQuery {
    lesson: String,
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
    Query(query): Query<C4DocQuery>,
) -> ApiResult<ComponentDocDto> {
    tracing::info!(element_id, lesson = query.lesson, "GET /api/synapse/c4-doc");
    let lesson_path: Vec<String> = query
        .lesson
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    match state.service.component_doc(&lesson_path, &element_id).await {
        Ok(doc) => Ok(Json(dto::to_component_doc(&doc))),
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
