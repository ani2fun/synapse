//! `/api/admin/content-editors`: list · grant · revoke, gated per call by the shared admin gate.
//!
//! Deliberately a SECOND list beside `/api/admin/allowlist` rather than a reuse of it. The submit
//! allowlist grants shared compute and storage; this one grants the ability to open pull requests
//! against a public repository under the deployment's own token. Revoking one must never be the
//! same act as revoking the other.
//!
//! The wire shape IS shared (`AllowlistEntryDto`/`GrantRequestDto`) — the two lists differ in
//! meaning, not in structure, which is what lets the admin page render both with one component.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get};
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::submission::{AllowlistEntryDto, GrantRequestDto};

use crate::authoring::application::ContentEditors;
use crate::authoring::http::{AuthoringRoutesState, dto};
use crate::platform::admin_gate::require_admin;

pub fn routes(state: AuthoringRoutesState) -> Router {
    Router::new()
        .route(
            "/api/admin/content-editors",
            get(list_content_editors).post(grant_content_editor),
        )
        .route(
            "/api/admin/content-editors/{username}",
            delete(revoke_content_editor),
        )
        .with_state(state)
}

async fn gate(
    state: &AuthoringRoutesState,
    headers: &HeaderMap,
) -> Result<crate::identity::domain::Username, dto::Reject> {
    require_admin(&state.identity, &state.admin_users, headers, "content-editors").await
}

/// The grants, newest first.
#[utoipa::path(
    get,
    path = "/api/admin/content-editors",
    operation_id = "listContentEditors",
    responses(
        (status = 200, description = "Every grant, newest first", body = [AllowlistEntryDto]),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn list_content_editors(
    State(state): State<AuthoringRoutesState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AllowlistEntryDto>>, dto::Reject> {
    gate(&state, &headers).await?;
    match state.editors.list().await {
        Ok(entries) => Ok(Json(entries.iter().map(dto::to_editor).collect())),
        Err(error) => Err(dto::to_error(&error)),
    }
}

/// Grant (upsert) — the stored row comes back; usernames are canonicalised here.
#[utoipa::path(
    post,
    path = "/api/admin/content-editors",
    operation_id = "grantContentEditor",
    request_body = GrantRequestDto,
    responses(
        (status = 200, description = "The stored grant", body = AllowlistEntryDto),
        (status = 400, description = "Blank username", body = ApiError),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn grant_content_editor(
    State(state): State<AuthoringRoutesState>,
    headers: HeaderMap,
    Json(request): Json<GrantRequestDto>,
) -> Result<Json<AllowlistEntryDto>, dto::Reject> {
    gate(&state, &headers).await?;
    // Canonical lowercase — the same shape the verifier emits and the propose gate compares.
    let username = request.username.trim().to_lowercase();
    if username.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Username required".to_owned(),
                detail: Some("A grant needs a non-blank username".to_owned()),
                hint: None,
            }),
        ));
    }
    let note = request.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    match state.editors.grant(&username, note).await {
        Ok(entry) => Ok(Json(dto::to_editor(&entry))),
        Err(error) => Err(dto::to_error(&error)),
    }
}

/// Revoke — 204 on removal, 404 when the grant never existed. Change requests the person already
/// opened are untouched: those live on the forge, and closing them is a reviewer's call.
#[utoipa::path(
    delete,
    path = "/api/admin/content-editors/{username}",
    operation_id = "revokeContentEditor",
    params(("username" = String, Path, description = "The granted username")),
    responses(
        (status = 204, description = "Revoked"),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError),
        (status = 404, description = "No such grant", body = ApiError)
    )
)]
pub(crate) async fn revoke_content_editor(
    State(state): State<AuthoringRoutesState>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> Result<StatusCode, dto::Reject> {
    gate(&state, &headers).await?;
    let username = raw.trim().to_lowercase();
    match state.editors.revoke(&username).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "No such grant".to_owned(),
                detail: Some(username),
                hint: None,
            }),
        )),
        Err(error) => Err(dto::to_error(&error)),
    }
}
