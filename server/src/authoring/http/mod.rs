//! The content-editing HTTP surface. `config` answers for everyone (including anonymous, so the
//! reader can decide whether to show the affordance at all); every other verb needs a verified
//! bearer AND a place on the content-editor allowlist.
//!
//! The bearer skeleton is `identity::http::optional_user`, which owns the never-silently-anonymous
//! rule; only the per-verb policy stays here. When the deployment has no forge configured this
//! whole router is never merged, so every path below is a structural 404 rather than a feature
//! flag checked per request.

pub mod admin;
mod dto;

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::authoring::{EditConfigDto, EditRequestDto, EditSourceDto, ProposeEditRequestDto};

use crate::authoring::application::{Editor, ProposeEdit};
use crate::authoring::infrastructure::{
    ConfiguredForges, FsLessonSource, PostgresContentEditors, PostgresEditRequests,
};
use crate::catalog::infrastructure::{FileSystemContentRepository, PostgresContentSources};
use crate::identity::http::LiveIdentityService;
use crate::platform::client_ip::{Peer, client_ip};
use crate::platform::rate_limiter::RateLimiter;

/// The production service: the filesystem lesson source, the Postgres allowlist and store, and
/// whichever forge the deployment configured.
pub type LiveProposeEdit = ProposeEdit<
    FsLessonSource<FileSystemContentRepository>,
    PostgresContentEditors,
    PostgresEditRequests,
    ConfiguredForges<PostgresContentSources>,
>;

pub struct AuthoringRoutesState {
    pub service: Arc<LiveProposeEdit>,
    pub identity: Arc<LiveIdentityService>,
    /// The same store the service gates on — the admin panel manages it through this handle.
    pub editors: Arc<PostgresContentEditors>,
    pub admin_users: Arc<std::collections::HashSet<String>>,
    pub limiter: Arc<RateLimiter>,
}

/// Hand-written: `#[derive(Clone)]` would demand `Clone` on members that do not promise it.
impl Clone for AuthoringRoutesState {
    fn clone(&self) -> Self {
        Self {
            service: Arc::clone(&self.service),
            identity: Arc::clone(&self.identity),
            editors: Arc::clone(&self.editors),
            admin_users: Arc::clone(&self.admin_users),
            limiter: Arc::clone(&self.limiter),
        }
    }
}

type ApiResult<T> = Result<Json<T>, dto::Reject>;

pub fn routes(state: AuthoringRoutesState) -> Router {
    Router::new()
        // `/config` and `/source/{*path}` do not overlap — the catch-all lives one segment
        // deeper — but the specific route is still registered first, as the catalog does.
        .route("/api/edits/config", get(get_config))
        .route("/api/edits/source/{*path}", get(get_source))
        .route("/api/edits", get(list_mine).post(propose_edit))
        // `with_state` FIRST, then merge: both sub-routers carry the same state type, and a
        // still-stateful router cannot absorb one that has already been finalised.
        .with_state(state.clone())
        .merge(admin::routes(state))
}

/// The caller as this context sees them: the lowercase username is the allowlist key.
async fn caller(state: &AuthoringRoutesState, headers: &HeaderMap) -> Result<Option<Editor>, dto::Reject> {
    let user = crate::identity::http::optional_user(&state.identity, headers).await?;
    Ok(user.map(|user| Editor {
        username: user.username,
    }))
}

fn segments(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether this deployment offers editing, and whether THIS caller may use it. Answers for
/// everyone — an anonymous reader gets `canEdit: false`, not a 401, because the lesson page asks
/// this before it knows who is reading.
#[utoipa::path(
    get,
    path = "/api/edits/config",
    operation_id = "getEditConfig",
    responses(
        (status = 200, description = "Editing coordinates for this caller", body = EditConfigDto),
        (status = 500, description = "The allowlist store failed", body = ApiError)
    )
)]
pub(crate) async fn get_config(
    State(state): State<AuthoringRoutesState>,
    headers: HeaderMap,
) -> ApiResult<EditConfigDto> {
    let editor = caller(&state, &headers).await?;
    let can_edit = state
        .service
        .may_edit(editor.as_ref())
        .await
        .map_err(|e| dto::to_error(&e))?;
    let target = state.service.target().await;
    Ok(Json(EditConfigDto {
        enabled: true,
        mode: state.service.mode().await.to_owned(),
        repo: target.repo.clone(),
        base_branch: target.base_branch.clone(),
        can_edit,
    }))
}

/// The lesson's file, whole — frontmatter fence included, because that is what gets committed.
#[utoipa::path(
    get,
    path = "/api/edits/source/{path}",
    operation_id = "getEditSource",
    params(("path" = String, Path, description = "category…/book/chapter…/lesson")),
    responses(
        (status = 200, description = "The editable source", body = EditSourceDto),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not a content editor", body = ApiError),
        (status = 404, description = "Not an editable page", body = ApiError)
    )
)]
pub(crate) async fn get_source(
    State(state): State<AuthoringRoutesState>,
    headers: HeaderMap,
    Path(path): Path<String>,
) -> ApiResult<EditSourceDto> {
    tracing::info!(path, "GET /api/edits/source");
    let editor = caller(&state, &headers).await?;
    match state.service.source_for(editor.as_ref(), &segments(&path)).await {
        Ok(source) => Ok(Json(dto::to_source(source))),
        Err(error) => Err(dto::to_error(&error)),
    }
}

/// Propose the edit: commit to this contributor's branch for this page and open (or reuse) a pull
/// request.
#[utoipa::path(
    post,
    path = "/api/edits",
    operation_id = "proposeEdit",
    request_body = ProposeEditRequestDto,
    responses(
        (status = 200, description = "The change request", body = EditRequestDto),
        (status = 400, description = "The proposed edit is not valid", body = ApiError),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not a content editor", body = ApiError),
        (status = 404, description = "Not an editable page", body = ApiError),
        (status = 409, description = "The page changed while editing", body = ApiError),
        (status = 429, description = "Rate limited", body = ApiError),
        (status = 502, description = "The content repository is unreachable", body = ApiError)
    )
)]
pub(crate) async fn propose_edit(
    State(state): State<AuthoringRoutesState>,
    headers: HeaderMap,
    peer: Peer,
    Json(request): Json<ProposeEditRequestDto>,
) -> ApiResult<EditRequestDto> {
    let editor = caller(&state, &headers).await?;
    // Metered like every other write that costs something downstream — here the cost is calls
    // against the forge's own quota, which is shared by every contributor.
    over_budget(&state, editor.as_ref(), &headers, peer)?;
    tracing::info!(path = request.lesson_path, "POST /api/edits");
    match state
        .service
        .propose(
            editor.as_ref(),
            &segments(&request.lesson_path),
            &request.source,
            &request.base_fingerprint,
            request.summary.as_deref(),
        )
        .await
    {
        Ok(proposal) => Ok(Json(dto::to_proposal(&proposal))),
        Err(error) => Err(dto::to_error(&error)),
    }
}

/// The caller's own change requests, newest first.
#[utoipa::path(
    get,
    path = "/api/edits",
    operation_id = "listMyEdits",
    responses(
        (status = 200, description = "The caller's change requests", body = [EditRequestDto]),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not a content editor", body = ApiError)
    )
)]
pub(crate) async fn list_mine(
    State(state): State<AuthoringRoutesState>,
    headers: HeaderMap,
) -> ApiResult<Vec<EditRequestDto>> {
    let editor = caller(&state, &headers).await?;
    let mode = state.service.mode().await;
    match state.service.mine(editor.as_ref()).await {
        // `reused: false` on a read: it describes the SUBMISSION that created the row, and this
        // is not one.
        Ok(rows) => Ok(Json(
            rows.iter().map(|r| dto::to_request(r, false, mode)).collect(),
        )),
        Err(error) => Err(dto::to_error(&error)),
    }
}

/// The authenticated bucket keyed by username, the anonymous one by IP — the same split the run
/// and submit routes use. An anonymous caller is refused by the gate a moment later anyway; being
/// metered first stops an unauthenticated flood from reaching the allowlist query at all.
fn over_budget(
    state: &AuthoringRoutesState,
    editor: Option<&Editor>,
    headers: &HeaderMap,
    Peer(socket): Peer,
) -> Result<(), dto::Reject> {
    let outcome = match editor {
        Some(editor) => state.limiter.consume_authenticated(&editor.username),
        None => state.limiter.consume_anonymous(&client_ip(headers, socket)),
    };
    outcome.map(|_| ()).map_err(|throttled| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiError {
                error: "Too many requests".to_owned(),
                detail: Some(throttled.to_string()),
                hint: Some("Your draft is saved in this browser — try again shortly.".to_owned()),
            }),
        )
    })
}
