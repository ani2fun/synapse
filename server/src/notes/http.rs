//! The notes HTTP surface: GET reads the caller's note for ONE problem (anonymous → an empty note,
//! store untouched), PUT replaces it (bearer REQUIRED — never silently anonymous), and the
//! collection DELETE erases all of theirs. The bearer skeleton is
//! `identity::http::optional_user`, the shape `canvas` and `progress` already share.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use synapse_shared::api::ApiError;
use synapse_shared::notes::{NoteDto, SaveNoteRequestDto};
use synapse_shared::submission::DeleteResultDto;

use crate::identity::http::LiveIdentityService;
use crate::notes::{NoteStore, NotesError, PostgresNoteStore, check_length};

#[derive(Clone)]
pub struct NotesRoutesState {
    pub notes: Arc<PostgresNoteStore>,
    pub identity: Arc<LiveIdentityService>,
}

type ApiResult<T> = Result<(StatusCode, Json<T>), (StatusCode, Json<ApiError>)>;

pub fn routes(state: NotesRoutesState) -> Router {
    Router::new()
        .route("/api/notes", get(get_note).put(save_note).delete(erase_all))
        .with_state(state)
}

async fn caller_user(
    state: &NotesRoutesState,
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

/// The context error flattens HERE and only here (RS001: erase at the EDGE).
fn to_error(error: &NotesError) -> (StatusCode, Json<ApiError>) {
    match error {
        NotesError::TooLong { .. } => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Note is too long".to_owned(),
                detail: Some(error.to_string()),
                hint: None,
            }),
        ),
        NotesError::StoreFailed(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: "Notes unavailable".to_owned(),
                detail: Some(detail.clone()),
                hint: None,
            }),
        ),
    }
}

fn segments(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// What a problem with no stored note reads as — one shape for "nothing yet" and "anonymous".
fn empty_note(path: Vec<String>) -> NoteDto {
    NoteDto {
        path,
        text: String::new(),
        updated_at: None,
    }
}

#[derive(Deserialize)]
pub(crate) struct NoteQuery {
    path: String,
}

/// The caller's note for one problem. Private: an anonymous caller gets an empty note and the
/// store is never touched (`list_entries`' exact policy).
#[utoipa::path(
    get,
    path = "/api/notes",
    operation_id = "getNote",
    params(("path" = String, Query, description = "The problem's directory-mirror path")),
    responses(
        (status = 200, description = "The caller's note (empty when there is none)", body = NoteDto),
        (status = 500, description = "Store failed", body = ApiError)
    )
)]
pub(crate) async fn get_note(
    State(state): State<NotesRoutesState>,
    headers: HeaderMap,
    Query(query): Query<NoteQuery>,
) -> ApiResult<NoteDto> {
    let path = segments(&query.path);
    let Some(user) = caller_user(&state, &headers).await? else {
        return Ok((StatusCode::OK, Json(empty_note(path))));
    };
    match state.notes.get(&user.id.0, &path.join("/")).await {
        Ok(note) => Ok((StatusCode::OK, Json(note.unwrap_or_else(|| empty_note(path))))),
        Err(error) => Err(to_error(&error)),
    }
}

/// Replace the caller's note for one problem. Empty text deletes it. Bearer required.
#[utoipa::path(
    put,
    path = "/api/notes",
    operation_id = "saveNote",
    request_body = SaveNoteRequestDto,
    responses(
        (status = 200, description = "The stored note", body = NoteDto),
        (status = 400, description = "Longer than the note limit", body = ApiError),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 500, description = "Store failed", body = ApiError)
    )
)]
pub(crate) async fn save_note(
    State(state): State<NotesRoutesState>,
    headers: HeaderMap,
    Json(request): Json<SaveNoteRequestDto>,
) -> ApiResult<NoteDto> {
    let Some(user) = caller_user(&state, &headers).await? else {
        return Err(needs_token("Saving a note"));
    };
    check_length(&request.text).map_err(|error| to_error(&error))?;
    let path: Vec<String> = request.path.into_iter().filter(|s| !s.is_empty()).collect();
    let joined = path.join("/");
    tracing::info!(path = joined, len = request.text.len(), "PUT /api/notes");
    match state.notes.put(&user.id.0, &joined, &request.text).await {
        Ok(note) => Ok((StatusCode::OK, Json(note.unwrap_or_else(|| empty_note(path))))),
        Err(error) => Err(to_error(&error)),
    }
}

/// Erase every note of the caller — the "erase my data" leg. Other stores survive.
#[utoipa::path(
    delete,
    path = "/api/notes",
    operation_id = "eraseNotes",
    responses(
        (status = 200, description = "Erased", body = DeleteResultDto),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 500, description = "Store failed", body = ApiError)
    )
)]
pub(crate) async fn erase_all(
    State(state): State<NotesRoutesState>,
    headers: HeaderMap,
) -> ApiResult<DeleteResultDto> {
    let Some(user) = caller_user(&state, &headers).await? else {
        return Err(needs_token("Erasing notes"));
    };
    match state.notes.erase_all_for(&user.id.0).await {
        Ok(deleted) => Ok((StatusCode::OK, Json(DeleteResultDto { deleted }))),
        Err(error) => Err(to_error(&error)),
    }
}
