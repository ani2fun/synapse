//! The submission HTTP surface: POST → 202 (bearer optional — anonymous submits, a BAD token
//! 401s, never silently anonymous), public GET poll, PRIVATE list (anonymous → `[]`, store
//! untouched), owner-only delete + erase-all. DTO↔domain mapping lives ONLY here; the bearer
//! skeleton is `identity::http::optional_user` — only the anonymous POLICY and the per-verb
//! 401 copy stay local.

pub mod admin;
mod dto;

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use synapse_shared::api::ApiError;
use synapse_shared::submission::{DeleteResultDto, SubmissionAcceptedDto, SubmissionDto, SubmitRequestDto};
use uuid::Uuid;

use crate::catalog::infrastructure::FileSystemContentRepository;
use crate::execution::http::over_budget;
use crate::execution::infrastructure::GoJudgeRunner;
use crate::identity::http::LiveIdentityService;
use crate::platform::client_ip::{Peer, client_ip};
use crate::platform::rate_limiter::RateLimiter;
use crate::submission::application::{SubmitSolution, Submitter};
use crate::submission::domain::SubmissionId;
use crate::submission::infrastructure::{
    FsProblemTests, PostgresSubmissionAllowlist, PostgresSubmissionRepository,
};

/// The production wiring: Postgres store · filesystem suites · the go-judge-backed runner ·
/// the Postgres allowlist.
pub type LiveSubmitSolution = SubmitSolution<
    PostgresSubmissionRepository,
    FsProblemTests<FileSystemContentRepository>,
    GoJudgeRunner,
    PostgresSubmissionAllowlist,
>;

#[derive(Clone)]
pub struct SubmissionRoutesState {
    pub submit: Arc<LiveSubmitSolution>,
    pub identity: Arc<LiveIdentityService>,
    pub limiter: Arc<RateLimiter>,
}

type ApiResult<T> = Result<(StatusCode, Json<T>), (StatusCode, Json<ApiError>)>;

pub fn routes(state: SubmissionRoutesState) -> Router {
    Router::new()
        .route(
            "/api/submissions",
            post(submit_solution).get(list_submissions).delete(erase_all),
        )
        .route(
            "/api/submissions/{id}",
            get(get_submission).delete(delete_submission),
        )
        .with_state(state)
}

/// The submission-local name for the shared skeleton (`identity::http::optional_user`, which
/// owns the never-silently-anonymous rule); this keeps the call sites short.
async fn caller_user(
    state: &SubmissionRoutesState,
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

/// Submit a solution — stored and judged in the background; poll the returned id.
#[utoipa::path(
    post,
    path = "/api/submissions",
    operation_id = "submitSolution",
    request_body = SubmitRequestDto,
    responses(
        (status = 202, description = "Stored; judging in background", body = SubmissionAcceptedDto),
        (status = 404, description = "Not a problem", body = ApiError),
        (status = 429, description = "Over the submission budget", body = ApiError)
    )
)]
pub(crate) async fn submit_solution(
    State(state): State<SubmissionRoutesState>,
    peer: Peer,
    headers: HeaderMap,
    Json(request): Json<SubmitRequestDto>,
) -> ApiResult<SubmissionAcceptedDto> {
    let submitter = caller_user(&state, &headers).await?.map(|user| Submitter {
        user_id: user.id.0,
        username: user.username,
    });
    // The rate-limit budget gate: signed-in meters per subject, anonymous per IP.
    let consumed = match &submitter {
        Some(s) => state.limiter.consume_authenticated(&s.user_id),
        None => state.limiter.consume_anonymous(&client_ip(&headers, peer.0)),
    };
    if let Err(throttled) = consumed {
        return Err(over_budget(throttled, "Sign in for a bigger submission budget."));
    }
    tracing::info!(path = request.path.join("/"), "POST /api/submissions");
    match state
        .submit
        .submit(request.path, request.language, request.source, submitter)
        .await
    {
        Ok(id) => Ok((
            StatusCode::ACCEPTED,
            Json(SubmissionAcceptedDto { id: id.to_string() }),
        )),
        Err(error) => Err(dto::to_error(&error)),
    }
}

/// Poll one submission (public — ids are unguessable UUIDs).
#[utoipa::path(
    get,
    path = "/api/submissions/{id}",
    operation_id = "getSubmission",
    params(("id" = String, Path, description = "The submission id")),
    responses(
        (status = 200, description = "The submission", body = SubmissionDto),
        (status = 400, description = "Not a submission id", body = ApiError),
        (status = 404, description = "Unknown submission", body = ApiError)
    )
)]
pub(crate) async fn get_submission(
    State(state): State<SubmissionRoutesState>,
    Path(raw): Path<String>,
) -> ApiResult<SubmissionDto> {
    let Ok(id) = raw.parse::<Uuid>() else {
        return Err(dto::bad_id(&raw));
    };
    match state.submit.get(SubmissionId(id)).await {
        Ok(submission) => Ok((StatusCode::OK, Json(dto::to_dto(&submission)))),
        Err(error) => Err(dto::to_error(&error)),
    }
}

#[derive(Deserialize)]
pub(crate) struct ListQuery {
    path: String,
}

/// The caller's OWN submissions for a lesson, newest first — private: anonymous callers get
/// `[]` and the store is never touched.
#[utoipa::path(
    get,
    path = "/api/submissions",
    operation_id = "listSubmissions",
    params(("path" = String, Query, description = "The lesson's directory-mirror path")),
    responses((status = 200, description = "The caller's submissions, newest first", body = [SubmissionDto]))
)]
pub(crate) async fn list_submissions(
    State(state): State<SubmissionRoutesState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> ApiResult<Vec<SubmissionDto>> {
    let Some(user) = caller_user(&state, &headers).await? else {
        return Ok((StatusCode::OK, Json(Vec::new())));
    };
    let segments: Vec<String> = query
        .path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    match state.submit.list_for(&segments, Some(&user.id.0)).await {
        Ok(submissions) => Ok((
            StatusCode::OK,
            Json(submissions.iter().map(dto::to_dto).collect()),
        )),
        Err(error) => Err(dto::to_error(&error)),
    }
}

/// Owner-only delete.
#[utoipa::path(
    delete,
    path = "/api/submissions/{id}",
    operation_id = "deleteSubmission",
    params(("id" = String, Path, description = "The submission id")),
    responses(
        (status = 200, description = "Deleted", body = DeleteResultDto),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Someone else's", body = ApiError)
    )
)]
pub(crate) async fn delete_submission(
    State(state): State<SubmissionRoutesState>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> ApiResult<DeleteResultDto> {
    let Some(user) = caller_user(&state, &headers).await? else {
        return Err(needs_token("Deleting a submission"));
    };
    let Ok(id) = raw.parse::<Uuid>() else {
        return Err(dto::bad_id(&raw));
    };
    match state.submit.delete(SubmissionId(id), &user.id.0).await {
        Ok(()) => Ok((StatusCode::OK, Json(DeleteResultDto { deleted: 1 }))),
        Err(error) => Err(dto::to_error(&error)),
    }
}

/// Erase every submission of the caller ("reset my data").
#[utoipa::path(
    delete,
    path = "/api/submissions",
    operation_id = "eraseSubmissions",
    responses(
        (status = 200, description = "Erased", body = DeleteResultDto),
        (status = 401, description = "Anonymous", body = ApiError)
    )
)]
pub(crate) async fn erase_all(
    State(state): State<SubmissionRoutesState>,
    headers: HeaderMap,
) -> ApiResult<DeleteResultDto> {
    let Some(user) = caller_user(&state, &headers).await? else {
        return Err(needs_token("Erasing submissions"));
    };
    match state.submit.erase_all_for(&user.id.0).await {
        Ok(deleted) => Ok((StatusCode::OK, Json(DeleteResultDto { deleted }))),
        Err(error) => Err(dto::to_error(&error)),
    }
}
