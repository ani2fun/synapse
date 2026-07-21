//! `POST /api/run`. A badly-running program is a 200 with a
//! non-`Accepted` status; the error channel is for the CALLER's mistakes (422/413), the
//! BACKEND's failures (503/502), and the budget (429). The gate is identity-aware: an absent
//! bearer meters per IP, a verified bearer per subject (bad tokens 401, never silently
//! anonymous), and the signed-in budget is deliberately bigger.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::execution::{RunRequest, RunResult};

use crate::execution::application::{ExecutionError, RunCodeService};
use crate::execution::infrastructure::GoJudgeRunner;
use crate::identity::http::{LiveIdentityService, optional_user};
use crate::platform::client_ip::{Peer, client_ip};
use crate::platform::rate_limiter::{RateLimiter, Throttled};

pub type LiveRunService = RunCodeService<GoJudgeRunner>;

#[derive(Clone)]
pub struct ExecutionRoutesState {
    pub run: Arc<LiveRunService>,
    pub identity: Arc<LiveIdentityService>,
    pub limiter: Arc<RateLimiter>,
}

pub fn routes(state: ExecutionRoutesState) -> Router {
    Router::new().route("/api/run", post(run_code)).with_state(state)
}

/// Run one snippet in the sandbox.
#[utoipa::path(
    post,
    path = "/api/run",
    operation_id = "runCode",
    request_body = RunRequest,
    responses(
        (status = 200, description = "The run's outcome (including failed programs)", body = RunResult),
        (status = 401, description = "Bad bearer token", body = ApiError),
        (status = 422, description = "Unknown language", body = ApiError),
        (status = 413, description = "Payload over the byte caps", body = ApiError),
        (status = 429, description = "Over the run budget", body = ApiError),
        (status = 502, description = "Backend failed", body = ApiError),
        (status = 503, description = "Backend unavailable", body = ApiError)
    )
)]
pub(crate) async fn run_code(
    State(state): State<ExecutionRoutesState>,
    peer: Peer,
    headers: HeaderMap,
    Json(request): Json<RunRequest>,
) -> Result<Json<RunResult>, (StatusCode, Json<ApiError>)> {
    // The gate first: resolve the caller (bad token → 401), then meter the right ledger.
    let subject = optional_user(&state.identity, &headers)
        .await?
        .map(|user| user.id.0);
    let consumed = match &subject {
        Some(sub) => state.limiter.consume_authenticated(sub),
        None => state.limiter.consume_anonymous(&client_ip(&headers, peer.0)),
    };
    if let Err(throttled) = consumed {
        return Err(over_budget(throttled, "Sign in for a bigger run budget."));
    }

    tracing::info!(language = request.language, "POST /api/run");
    match state.run.run(&request).await {
        Ok(result) => Ok(Json(result)),
        Err(error) => Err(to_error(&error)),
    }
}

/// 429 with the retry seconds in the BODY — the uniform `(status, ApiError)` envelope, no
/// `Retry-After` header (deliberate: every error response uses the same envelope shape rather
/// than splitting rate-limit info across a header and a body).
pub(crate) fn over_budget(throttled: Throttled, hint: &str) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(ApiError {
            error: "Rate limit exceeded".to_owned(),
            detail: Some(format!("Retry after {}s", throttled.retry_after_sec)),
            hint: Some(hint.to_owned()),
        }),
    )
}

fn to_error(error: &ExecutionError) -> (StatusCode, Json<ApiError>) {
    let (status, message, detail, hint) = match error {
        ExecutionError::UnknownLanguage(alias) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("Language '{alias}' is not runnable"),
            None,
            None,
        ),
        ExecutionError::PayloadTooLarge { field, bytes, limit } => (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("{field} too large"),
            Some(format!("{bytes} bytes exceeds the {limit}-byte cap")),
            None,
        ),
        ExecutionError::BackendUnavailable(detail) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Execution backend unavailable".to_owned(),
            Some(detail.clone()),
            Some("Is go-judge running? Set EXECUTOR_URL.".to_owned()),
        ),
        ExecutionError::BackendFailed(detail) => (
            StatusCode::BAD_GATEWAY,
            "Execution backend failed".to_owned(),
            Some(detail.clone()),
            None,
        ),
    };
    (
        status,
        Json(ApiError {
            error: message,
            detail,
            hint,
        }),
    )
}
