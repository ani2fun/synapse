//! `POST /api/run`. A badly-running program is a 200 with a
//! non-`Accepted` status; the error channel is for the CALLER's mistakes (422/413), the
//! BACKEND's failures (503/502), the budget (429) and a full sandbox (503).
//!
//! Running code needs an ACCOUNT: an absent bearer is a 401 before anything is spent, and a bad
//! one is a 401 too (never silently anonymous). Anyone could otherwise point the sandbox at
//! anything, and an address-keyed budget is only as strong as the cheapest way to get a new
//! address. The anonymous ledger and tier stay in the crate — submissions still use them.
//!
//! Three gates follow sign-in, in this order, because each is cheaper to fail than the next is
//! to pass: ADMISSION (runs in flight — refused outright, spending nothing), then the RATE
//! budget, then the run itself at the signed-in TIER.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::execution::{RunRequest, RunResult};

use crate::execution::application::{ExecutionError, RunCodeService};
use crate::execution::domain::Tier;
use crate::execution::infrastructure::GoJudgeRunner;
use crate::identity::http::{LiveIdentityService, optional_user};
use crate::platform::admission::{Admission, Refused};
use crate::platform::rate_limiter::{RateLimiter, ThrottleScope, Throttled};

pub type LiveRunService = RunCodeService<GoJudgeRunner>;

#[derive(Clone)]
pub struct ExecutionRoutesState {
    pub run: Arc<LiveRunService>,
    pub identity: Arc<LiveIdentityService>,
    pub limiter: Arc<RateLimiter>,
    pub admission: Arc<Admission>,
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
        (status = 401, description = "No bearer token, or a bad one", body = ApiError),
        (status = 422, description = "Unknown language", body = ApiError),
        (status = 413, description = "Payload over the byte caps", body = ApiError),
        (status = 429, description = "Over the run budget, or the caller already has runs in flight", body = ApiError),
        (status = 502, description = "Backend failed", body = ApiError),
        (status = 503, description = "Backend unavailable, or the sandbox queue is full", body = ApiError)
    )
)]
pub(crate) async fn run_code(
    State(state): State<ExecutionRoutesState>,
    headers: HeaderMap,
    Json(request): Json<RunRequest>,
) -> Result<Json<RunResult>, (StatusCode, Json<ApiError>)> {
    // The caller first (no token or a bad one → 401), then the three gates.
    let Some(user) = optional_user(&state.identity, &headers).await? else {
        return Err(needs_sign_in());
    };
    let subject = user.id.0;
    let tier = Tier::SignedIn;
    let ticket = state
        .admission
        .admit(&format!("auth:{subject}"))
        .map_err(not_admitted)?;
    if let Err(throttled) = state.limiter.consume_authenticated(&subject) {
        return Err(over_budget(throttled, "Wait for the window to reset."));
    }

    tracing::info!(
        language = request.language,
        ?tier,
        in_flight = state.admission.in_flight(),
        "POST /api/run"
    );
    // DETACHED from the connection, holding the ticket. A client that disconnects drops this
    // handler's future, and the ticket with it — but go-judge is still running the program, so
    // the place it held would come back while the sandbox is still busy, and a caller who
    // disconnects on purpose could keep it full indefinitely. The task keeps the place until
    // the sandbox is actually done.
    let run = Arc::clone(&state.run);
    let finished = tokio::spawn(async move {
        let _ticket = ticket;
        run.run(&request, tier).await
    })
    .await;
    match finished {
        Ok(Ok(result)) => Ok(Json(result)),
        Ok(Err(error)) => Err(to_error(&error)),
        Err(join) => {
            tracing::error!(%join, "run task did not finish");
            Err(to_error(&ExecutionError::BackendFailed(
                "the run did not finish".to_owned(),
            )))
        }
    }
}

/// An anonymous run. Refused before admission and the budget, so it costs the server nothing.
fn needs_sign_in() -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(ApiError {
            error: "Sign in to run code".to_owned(),
            detail: None,
            hint: Some("Running code needs an account.".to_owned()),
        }),
    )
}

/// A run the sandbox has no room for. The caller's own limit is a 429 — theirs to wait out —
/// and a full queue a 503, because nothing about this caller caused it.
fn not_admitted(refused: Refused) -> (StatusCode, Json<ApiError>) {
    match refused {
        Refused::CallerBusy { limit } => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiError {
                error: "Too many runs in flight".to_owned(),
                detail: Some(format!(
                    "You already have {limit} run(s) going — wait for one to finish."
                )),
                hint: None,
            }),
        ),
        Refused::SandboxBusy => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError {
                error: "The sandbox is busy".to_owned(),
                detail: Some("Retry in a few seconds.".to_owned()),
                hint: Some("Many people are running code right now.".to_owned()),
            }),
        ),
    }
}

/// 429 with the retry seconds in the BODY — the uniform `(status, ApiError)` envelope, no
/// `Retry-After` header (deliberate: every error response uses the same envelope shape rather
/// than splitting rate-limit info across a header and a body).
///
/// `hint` is for the caller's OWN budget. The anonymous ceiling says its own thing: that caller
/// may not have run anything at all, and "you are over your budget" would be untrue.
pub(crate) fn over_budget(throttled: Throttled, hint: &str) -> (StatusCode, Json<ApiError>) {
    let (error, hint) = match throttled.scope {
        ThrottleScope::Caller => ("Rate limit exceeded", hint),
        ThrottleScope::AllAnonymous => (
            "Too many anonymous requests right now",
            "Sign in to use your own budget.",
        ),
    };
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(ApiError {
            error: error.to_owned(),
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
