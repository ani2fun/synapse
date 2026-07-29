//! `/api/admin/content-sources`: list · register · remove, gated per call by the shared admin gate.
//!
//! This is the surface that makes a satellite guide repo a row instead of a redeploy. The primary
//! checkout is deliberately absent: it arrives by git-sync, is always mounted, and is always first
//! — which is what makes the merge's first-wins rule safe while a book lives in two places.

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::catalog::{CatalogWarningDto, ContentSourceDto, RegisterContentSourceDto};

use crate::catalog::application::{
    ContentSourceDraft, ContentSourceRecord, ContentSources, RegistryError, grouping_to_string,
};
use crate::catalog::domain::catalog::CatalogWarning;
use crate::catalog::http::routes::LiveCatalogService;
use crate::catalog::infrastructure::SyncTrigger;
use crate::identity::http::LiveIdentityService;
use crate::platform::admin_gate::{Reject, require_admin};

pub struct ContentSourceRoutesState<S> {
    pub sources: Arc<S>,
    pub identity: Arc<LiveIdentityService>,
    pub admin_users: Arc<HashSet<String>>,
    /// The live catalog, for the conflicts it had to resolve. Concrete rather than generic: one
    /// implementation exists, and a type parameter here would spread through the C4 routes that
    /// share this state for no variation anyone uses.
    pub catalog: Arc<LiveCatalogService>,
    /// Absent when the reconcile loop is disabled (`content_sync_seconds = 0`, which is how the
    /// tests and the single-checkout deployment run). "Sync now" then answers honestly instead of
    /// pretending to have queued something.
    pub sync: Option<SyncTrigger>,
}

/// Hand-written: `#[derive(Clone)]` would demand `Clone` on `S`, which no adapter promises.
impl<S> Clone for ContentSourceRoutesState<S> {
    fn clone(&self) -> Self {
        Self {
            sources: Arc::clone(&self.sources),
            identity: Arc::clone(&self.identity),
            admin_users: Arc::clone(&self.admin_users),
            catalog: Arc::clone(&self.catalog),
            sync: self.sync.clone(),
        }
    }
}

pub fn routes<S: ContentSources + 'static>(state: ContentSourceRoutesState<S>) -> Router {
    Router::new()
        .route(
            "/api/admin/content-sources",
            get(list_content_sources::<S>).post(register_content_source::<S>),
        )
        .route("/api/admin/content-sources/sync", post(sync_now::<S>))
        .route(
            "/api/admin/content-sources/{id}",
            delete(remove_content_source::<S>),
        )
        .route("/api/admin/content-warnings", get(content_warnings::<S>))
        .with_state(state)
}

/// Flatten the merge's vocabulary into something a panel can render without knowing the rules.
fn warning_to_dto(warning: &CatalogWarning) -> CatalogWarningDto {
    match warning {
        CatalogWarning::DuplicateBookSlug {
            slug,
            kept_source,
            skipped_source,
        } => CatalogWarningDto {
            kind: "duplicateBookSlug".to_owned(),
            slug: Some(slug.clone()),
            sources: vec![kept_source.clone(), skipped_source.clone()],
            detail: format!(
                "Two sources carry the book “{slug}”. {kept_source} is serving it; {skipped_source}'s copy is ignored until {kept_source} stops shipping it."
            ),
        },
        CatalogWarning::CategoryRedeclared {
            slug,
            kept_source,
            ignored_source,
        } => CatalogWarningDto {
            kind: "categoryRedeclared".to_owned(),
            slug: Some(slug.clone()),
            sources: vec![kept_source.clone(), ignored_source.clone()],
            detail: format!(
                "Both {kept_source} and {ignored_source} declare the category “{slug}”. {kept_source}'s category.json wins."
            ),
        },
        CatalogWarning::BookSourceWithoutSlug { source_id } => CatalogWarningDto {
            kind: "bookSourceWithoutSlug".to_owned(),
            slug: None,
            sources: vec![source_id.clone()],
            detail: format!(
                "{source_id} is a book repository with no slug in book.json, so its URL fell back to the repository name. Set the slug — it IS the URL."
            ),
        },
    }
}

/// What the merge resolved across sources. The signal that makes a migration safe: while a book
/// exists in two repositories this names the one actually serving.
#[utoipa::path(
    get,
    path = "/api/admin/content-warnings",
    operation_id = "listContentWarnings",
    responses(
        (status = 200, description = "Conflicts the current catalog resolved", body = [CatalogWarningDto]),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn content_warnings<S: ContentSources>(
    State(state): State<ContentSourceRoutesState<S>>,
    headers: HeaderMap,
) -> Result<Json<Vec<CatalogWarningDto>>, Reject> {
    gate(&state, &headers).await?;
    match state.catalog.warnings().await {
        Ok(warnings) => Ok(Json(warnings.iter().map(warning_to_dto).collect())),
        Err(error) => {
            tracing::error!(%error, "content warnings unavailable");
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError {
                    error: "The catalog could not be read".to_owned(),
                    detail: None,
                    hint: None,
                }),
            ))
        }
    }
}

/// Ask the reconcile loop to run now. `202`, not `200`: the fetch is the loop's, and a repository
/// that is slow or unreachable must not hold this request open.
#[utoipa::path(
    post,
    path = "/api/admin/content-sources/sync",
    operation_id = "syncContentSources",
    responses(
        (status = 202, description = "A reconcile was requested"),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError),
        (status = 503, description = "The reconcile loop is not running", body = ApiError)
    )
)]
pub(crate) async fn sync_now<S: ContentSources>(
    State(state): State<ContentSourceRoutesState<S>>,
    headers: HeaderMap,
) -> Result<StatusCode, Reject> {
    gate(&state, &headers).await?;
    let Some(trigger) = state.sync.as_ref() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError {
                error: "The content sync loop is disabled".to_owned(),
                detail: None,
                hint: Some("set SYNAPSE_CONTENT_SYNC_SECONDS above 0".to_owned()),
            }),
        ));
    };
    trigger.notify_one();
    Ok(StatusCode::ACCEPTED)
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
    // DTO in, command out — what a blank branch or an absent `enabled` MEANS is the application's
    // to decide, and `register` is where it decides it.
    let draft = ContentSourceDraft::register(
        &request.repo,
        request.branch.as_deref(),
        request.grouping.as_deref(),
        request.order,
        request.enabled,
    )
    .map_err(|error| to_error(&error))?;
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
