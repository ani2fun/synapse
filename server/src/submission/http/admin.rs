//! `/api/admin/allowlist`: list · grant · revoke, gated per call — the ADMIN check is config
//! (`ADMIN_USERS`), not a token claim, and the server re-checks EVERY request (`MeDto.admin`
//! is UX only). Generic over the allowlist port so the route tests drive a fake through the
//! real router.

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get};
use axum::{Json, Router};
use chrono::SecondsFormat;
use synapse_shared::api::ApiError;
use synapse_shared::submission::{AllowlistEntryDto, GrantRequestDto};

use crate::identity::http::LiveIdentityService;
use crate::platform::admin_gate::{Reject, require_admin as gate};
use crate::submission::application::{AllowlistEntry, SubmissionAllowlist};

pub struct AdminRoutesState<L> {
    pub allowlist: Arc<L>,
    pub identity: Arc<LiveIdentityService>,
    /// Lowercase usernames from `ADMIN_USERS` — compared against the verifier's canonical
    /// lowercase output, apples to apples.
    pub admin_users: Arc<HashSet<String>>,
}

impl<L> Clone for AdminRoutesState<L> {
    fn clone(&self) -> Self {
        Self {
            allowlist: Arc::clone(&self.allowlist),
            identity: Arc::clone(&self.identity),
            admin_users: Arc::clone(&self.admin_users),
        }
    }
}

pub fn routes<L: SubmissionAllowlist + 'static>(state: AdminRoutesState<L>) -> Router {
    Router::new()
        .route(
            "/api/admin/allowlist",
            get(list_allowlist::<L>).post(grant_allowlist::<L>),
        )
        .route("/api/admin/allowlist/{username}", delete(revoke_allowlist::<L>))
        .with_state(state)
}

/// The gate itself lives in `platform::admin_gate`, shared with the readership read. The
/// invariant it carries: ADMIN is config, re-checked here on every call.
async fn require_admin<L>(state: &AdminRoutesState<L>, headers: &HeaderMap) -> Result<String, Reject> {
    gate(&state.identity, &state.admin_users, headers, "allowlist").await
}

fn to_dto(entry: &AllowlistEntry) -> AllowlistEntryDto {
    AllowlistEntryDto {
        username: entry.username.clone(),
        note: entry.note.clone(),
        granted_at: entry.granted_at.to_rfc3339_opts(SecondsFormat::Millis, true),
    }
}

/// The grants, newest first.
#[utoipa::path(
    get,
    path = "/api/admin/allowlist",
    operation_id = "listAllowlist",
    responses(
        (status = 200, description = "Every grant, newest first", body = [AllowlistEntryDto]),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn list_allowlist<L: SubmissionAllowlist>(
    State(state): State<AdminRoutesState<L>>,
    headers: HeaderMap,
) -> Result<Json<Vec<AllowlistEntryDto>>, Reject> {
    require_admin(&state, &headers).await?;
    match state.allowlist.list().await {
        Ok(entries) => Ok(Json(entries.iter().map(to_dto).collect())),
        Err(error) => Err(super::dto::to_error(&error)),
    }
}

/// Grant (upsert) — the stored row comes back; usernames are canonicalised here.
#[utoipa::path(
    post,
    path = "/api/admin/allowlist",
    operation_id = "grantAllowlist",
    request_body = GrantRequestDto,
    responses(
        (status = 200, description = "The stored grant", body = AllowlistEntryDto),
        (status = 400, description = "Blank username", body = ApiError),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn grant_allowlist<L: SubmissionAllowlist>(
    State(state): State<AdminRoutesState<L>>,
    headers: HeaderMap,
    Json(request): Json<GrantRequestDto>,
) -> Result<Json<AllowlistEntryDto>, Reject> {
    require_admin(&state, &headers).await?;
    // Canonical lowercase — the same shape the verifier emits and the gate compares.
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
    match state.allowlist.grant(&username, note).await {
        Ok(entry) => Ok(Json(to_dto(&entry))),
        Err(error) => Err(super::dto::to_error(&error)),
    }
}

/// Revoke — 204 on removal, 404 when the grant never existed.
#[utoipa::path(
    delete,
    path = "/api/admin/allowlist/{username}",
    operation_id = "revokeAllowlist",
    params(("username" = String, Path, description = "The granted username")),
    responses(
        (status = 204, description = "Revoked"),
        (status = 404, description = "No such grant", body = ApiError),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn revoke_allowlist<L: SubmissionAllowlist>(
    State(state): State<AdminRoutesState<L>>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> Result<StatusCode, Reject> {
    require_admin(&state, &headers).await?;
    let username = raw.trim().to_lowercase();
    match state.allowlist.revoke(&username).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "No such grant".to_owned(),
                detail: Some(username),
                hint: None,
            }),
        )),
        Err(error) => Err(super::dto::to_error(&error)),
    }
}
