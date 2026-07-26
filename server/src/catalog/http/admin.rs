//! `/api/admin/content-sources`: list · register · remove, gated per call by the shared admin gate.
//!
//! This is the surface that makes a satellite guide repo a row instead of a redeploy. The primary
//! checkout is deliberately absent: it arrives by git-sync, is always mounted, and is always first
//! — which is what makes the merge's first-wins rule safe while a book lives in two places.

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get};
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::catalog::{ContentSourceDto, RegisterContentSourceDto};

use crate::catalog::application::{
    ContentSourceDraft, ContentSourceRecord, ContentSources, RegistryError, grouping_from_str,
    grouping_to_string,
};
use crate::identity::http::LiveIdentityService;
use crate::platform::admin_gate::{Reject, require_admin};

pub struct ContentSourceRoutesState<S> {
    pub sources: Arc<S>,
    pub identity: Arc<LiveIdentityService>,
    pub admin_users: Arc<HashSet<String>>,
}

/// Hand-written: `#[derive(Clone)]` would demand `Clone` on `S`, which no adapter promises.
impl<S> Clone for ContentSourceRoutesState<S> {
    fn clone(&self) -> Self {
        Self {
            sources: Arc::clone(&self.sources),
            identity: Arc::clone(&self.identity),
            admin_users: Arc::clone(&self.admin_users),
        }
    }
}

pub fn routes<S: ContentSources + 'static>(state: ContentSourceRoutesState<S>) -> Router {
    Router::new()
        .route(
            "/api/admin/content-sources",
            get(list_content_sources::<S>).post(register_content_source::<S>),
        )
        .route(
            "/api/admin/content-sources/{id}",
            delete(remove_content_source::<S>),
        )
        .with_state(state)
}

fn to_dto(record: &ContentSourceRecord) -> ContentSourceDto {
    ContentSourceDto {
        id: record.id.clone(),
        repo: record.repo.clone(),
        branch: record.branch.clone(),
        grouping: grouping_to_string(&record.grouping),
        order: record.order,
        enabled: record.enabled,
        last_sha: record.last_sha.clone(),
        last_synced_at: record.last_synced_at.map(|t| t.to_rfc3339()),
        last_error: record.last_error.clone(),
    }
}

/// `Invalid` is the caller's fault and says so; `StoreFailed` stays opaque.
fn to_error(error: &RegistryError) -> Reject {
    match error {
        RegistryError::Invalid(detail) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Invalid content source".to_owned(),
                detail: Some(detail.clone()),
                hint: Some("repo is owner/name; grouping segments are slug-like".to_owned()),
            }),
        ),
        RegistryError::StoreFailed(detail) => {
            tracing::error!(%detail, "content source store failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: "Content source store unavailable".to_owned(),
                    detail: None,
                    hint: None,
                }),
            )
        }
    }
}

async fn gate<S: ContentSources>(
    state: &ContentSourceRoutesState<S>,
    headers: &HeaderMap,
) -> Result<String, Reject> {
    require_admin(&state.identity, &state.admin_users, headers, "content-sources").await
}

/// Every registered repository, in mount order, with its sync state.
#[utoipa::path(
    get,
    path = "/api/admin/content-sources",
    operation_id = "listContentSources",
    responses(
        (status = 200, description = "Registered repositories, in mount order", body = [ContentSourceDto]),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn list_content_sources<S: ContentSources>(
    State(state): State<ContentSourceRoutesState<S>>,
    headers: HeaderMap,
) -> Result<Json<Vec<ContentSourceDto>>, Reject> {
    gate(&state, &headers).await?;
    match state.sources.list().await {
        Ok(records) => Ok(Json(records.iter().map(to_dto).collect())),
        Err(error) => Err(to_error(&error)),
    }
}

/// Register (upsert). The book appears once the next fetch lands it, not on this call.
#[utoipa::path(
    post,
    path = "/api/admin/content-sources",
    operation_id = "registerContentSource",
    request_body = RegisterContentSourceDto,
    responses(
        (status = 200, description = "The stored registration", body = ContentSourceDto),
        (status = 400, description = "Malformed repo or grouping", body = ApiError),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn register_content_source<S: ContentSources>(
    State(state): State<ContentSourceRoutesState<S>>,
    headers: HeaderMap,
    Json(request): Json<RegisterContentSourceDto>,
) -> Result<Json<ContentSourceDto>, Reject> {
    gate(&state, &headers).await?;
    let draft = ContentSourceDraft {
        repo: request.repo.trim().to_owned(),
        branch: request
            .branch
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .unwrap_or("main")
            .to_owned(),
        grouping: grouping_from_str(request.grouping.as_deref().unwrap_or_default()),
        order: request.order,
        enabled: request.enabled.unwrap_or(true),
    };
    match state.sources.upsert(&draft).await {
        Ok(record) => Ok(Json(to_dto(&record))),
        Err(error) => Err(to_error(&error)),
    }
}

/// Forget a repository — 204 on removal, 404 when it was never registered. The cached checkout is
/// reclaimed by the fetch loop; nothing is deleted on the forge.
#[utoipa::path(
    delete,
    path = "/api/admin/content-sources/{id}",
    operation_id = "removeContentSource",
    params(("id" = String, Path, description = "The derived source id")),
    responses(
        (status = 204, description = "Removed"),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError),
        (status = 404, description = "No such source", body = ApiError)
    )
)]
pub(crate) async fn remove_content_source<S: ContentSources>(
    State(state): State<ContentSourceRoutesState<S>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, Reject> {
    gate(&state, &headers).await?;
    match state.sources.remove(id.trim()).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "No such content source".to_owned(),
                detail: Some(id),
                hint: None,
            }),
        )),
        Err(error) => Err(to_error(&error)),
    }
}
