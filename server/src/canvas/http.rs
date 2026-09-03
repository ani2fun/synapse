//! The canvas HTTP surface: POST saves an entry (bearer REQUIRED — never silently anonymous),
//! GET lists the caller's entries for ONE problem (anonymous → `[]`, store untouched), DELETE
//! removes one the caller owns, and the collection DELETE erases all of theirs. The bearer
//! skeleton is `identity::http::optional_user`; only the anonymous policy and the per-verb 401
//! copy stay local — the shape `progress` and `submission` already share.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use synapse_shared::api::ApiError;
use synapse_shared::canvas::{CanvasEntryDto, SaveCanvasRequestDto};
use synapse_shared::submission::DeleteResultDto;

use crate::canvas::{CanvasError, CanvasStore, PostgresCanvasStore};
use crate::identity::http::LiveIdentityService;

#[derive(Clone)]
pub struct CanvasRoutesState {
    pub canvas: Arc<PostgresCanvasStore>,
    pub identity: Arc<LiveIdentityService>,
}

type ApiResult<T> = Result<(StatusCode, Json<T>), (StatusCode, Json<ApiError>)>;

pub fn routes(state: CanvasRoutesState) -> Router {
    Router::new()
        .route(
            "/api/canvas",
            post(save_entry).get(list_entries).delete(erase_all),
        )
        .route("/api/canvas/{id}", axum::routing::delete(delete_entry))
        .with_state(state)
}

/// The canvas-local name for the shared bearer skeleton (`identity::http::optional_user`, which
/// owns the never-silently-anonymous rule).
async fn caller_user(
    state: &CanvasRoutesState,
    headers: &HeaderMap,
) -> Result<Option<crate::identity::domain::AuthenticatedUser>, (StatusCode, Json<ApiError>)> {
    crate::identity::http::optional_user(&state.identity, headers).await
}

fn needs_token(verb: &str) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(ApiError {
            error: format!("{verb} requires a bearer token"),
            detail: Some("Sign in first".to_owned()),
            hint: None,
        }),
    )
}

/// The context error flattens HERE and only here (RS001: erase at the EDGE) — the two access
/// variants stay distinct all the way out, so a reader is told "no such entry" or "not yours"
/// rather than one generic denial.
fn to_error(error: &CanvasError) -> (StatusCode, Json<ApiError>) {
    match error {
        CanvasError::StoreFailed(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: "Canvas unavailable".to_owned(),
                detail: Some(detail.clone()),
                hint: None,
            }),
        ),
        CanvasError::NotFound(id) => (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "No such canvas entry".to_owned(),
                detail: Some(id.clone()),
                hint: None,
            }),
        ),
        CanvasError::NotYours(id) => (
            StatusCode::FORBIDDEN,
            Json(ApiError {
                error: "That canvas entry is not yours".to_owned(),
                detail: Some(id.clone()),
                hint: None,
            }),
        ),
    }
}

fn joined(segments: &[String]) -> String {
    segments
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("/")
}

/// Save one canvas entry for the caller. Bearer required — a plan saved to nobody is a plan lost.
#[utoipa::path(
    post,
    path = "/api/canvas",
    operation_id = "saveCanvasEntry",
    request_body = SaveCanvasRequestDto,
    responses(
        (status = 201, description = "The stored entry", body = CanvasEntryDto),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 500, description = "Store failed", body = ApiError)
    )
)]
pub(crate) async fn save_entry(
    State(state): State<CanvasRoutesState>,
    headers: HeaderMap,
    Json(request): Json<SaveCanvasRequestDto>,
) -> ApiResult<CanvasEntryDto> {
    let Some(user) = caller_user(&state, &headers).await? else {
        return Err(needs_token("Saving a canvas entry"));
    };
    let path = joined(&request.path);
    tracing::info!(path = path, "POST /api/canvas");
    match state.canvas.save(&user.id.0, &path, &request.body).await {
        Ok(entry) => Ok((StatusCode::CREATED, Json(entry))),
        Err(error) => Err(to_error(&error)),
    }
}

#[derive(Deserialize)]
pub(crate) struct ListQuery {
    path: String,
}

/// The caller's OWN entries for one problem, newest first — private: anonymous callers get `[]`
/// and the store is never touched (`list_submissions`' exact policy).
#[utoipa::path(
    get,
    path = "/api/canvas",
    operation_id = "listCanvasEntries",
    params(("path" = String, Query, description = "The problem's directory-mirror path")),
    responses((status = 200, description = "The caller's entries, newest first", body = [CanvasEntryDto]))
)]
pub(crate) async fn list_entries(
    State(state): State<CanvasRoutesState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> ApiResult<Vec<CanvasEntryDto>> {
    let Some(user) = caller_user(&state, &headers).await? else {
        return Ok((StatusCode::OK, Json(Vec::new())));
    };
    let segments: Vec<String> = query
        .path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    match state.canvas.list_for(&user.id.0, &joined(&segments)).await {
        Ok(entries) => Ok((StatusCode::OK, Json(entries))),
        Err(error) => Err(to_error(&error)),
    }
}

/// Owner-only delete.
#[utoipa::path(
    delete,
    path = "/api/canvas/{id}",
    operation_id = "deleteCanvasEntry",
    params(("id" = String, Path, description = "The entry id")),
    responses(
        (status = 200, description = "Deleted", body = DeleteResultDto),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Someone else's", body = ApiError),
        (status = 404, description = "Unknown entry", body = ApiError)
    )
)]
pub(crate) async fn delete_entry(
    State(state): State<CanvasRoutesState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<DeleteResultDto> {
    let Some(user) = caller_user(&state, &headers).await? else {
        return Err(needs_token("Deleting a canvas entry"));
    };
    match state.canvas.delete(&user.id.0, &id).await {
        Ok(()) => Ok((StatusCode::OK, Json(DeleteResultDto { deleted: 1 }))),
        Err(error) => Err(to_error(&error)),
    }
}

/// Erase every canvas entry of the caller — the "reset my data" leg. Submissions and progress are
/// separate stores and survive.
#[utoipa::path(
    delete,
    path = "/api/canvas",
    operation_id = "eraseCanvasEntries",
    responses(
        (status = 200, description = "Erased", body = DeleteResultDto),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 500, description = "Store failed", body = ApiError)
    )
)]
pub(crate) async fn erase_all(
    State(state): State<CanvasRoutesState>,
    headers: HeaderMap,
) -> ApiResult<DeleteResultDto> {
    let Some(user) = caller_user(&state, &headers).await? else {
        return Err(needs_token("Erasing canvas entries"));
    };
    match state.canvas.erase_all_for(&user.id.0).await {
        Ok(deleted) => Ok((StatusCode::OK, Json(DeleteResultDto { deleted }))),
        Err(error) => Err(to_error(&error)),
    }
}
