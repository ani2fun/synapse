//! The three catalog endpoints (oracle: `CatalogEndpoints` + `CatalogRoutes`). Route shape
//! matters: `/index` and `/c4-doc/{id}` are more specific than the `{*paths}` lesson catch-all,
//! and axum's router picks the most specific match.

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

/// The production service: the catalog over the filesystem adapter (wired in `main`).
pub type LiveCatalogService = CatalogService<FileSystemContentRepository>;

type CatalogState = State<Arc<LiveCatalogService>>;
type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

pub fn routes(service: Arc<LiveCatalogService>) -> Router {
    Router::new()
        .route("/api/synapse/index", get(get_synapse_index))
        .route("/api/synapse/c4-doc/{element_id}", get(get_component_doc))
        .route("/api/synapse/{*paths}", get(get_synapse_lesson))
        .with_state(service)
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
pub async fn get_synapse_index(State(service): CatalogState) -> ApiResult<SynapseIndexDto> {
    tracing::info!("GET /api/synapse/index");
    match service.index().await {
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
pub async fn get_component_doc(
    State(service): CatalogState,
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
    match service.component_doc(&lesson_path, &element_id).await {
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
pub async fn get_synapse_lesson(
    State(service): CatalogState,
    Path(paths): Path<String>,
) -> ApiResult<LessonPayloadDto> {
    tracing::info!(path = paths, "GET /api/synapse/{{lesson}}");
    let segments: Vec<String> = paths
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    match service.lesson(&segments).await {
        Ok(content) => Ok(Json(dto::to_payload(&content))),
        Err(error) => fail(&error),
    }
}
