//! The tutor HTTP surface (oracle: `TutorRoutes`): `config` ALWAYS answers; `chat` is only
//! MOUNTED when enabled — a disabled deployment 404s it structurally (no runtime check, no
//! `Disabled` error case). Generic over the client port so route ITs drive a fake.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::tutor::{TutorChatRequestDto, TutorChatResponseDto, TutorConfigDto};

use crate::tutoring::application::{ChatContext, TutorClient, TutorError, TutoringService};
use crate::tutoring::infrastructure::OllamaTutorClient;

pub type LiveTutoringService = TutoringService<OllamaTutorClient>;

pub struct TutorRoutesState<C> {
    pub service: Arc<TutoringService<C>>,
    pub enabled: bool,
    pub model: String,
}

impl<C> Clone for TutorRoutesState<C> {
    fn clone(&self) -> Self {
        Self {
            service: Arc::clone(&self.service),
            enabled: self.enabled,
            model: self.model.clone(),
        }
    }
}

pub fn routes<C: TutorClient + 'static>(state: TutorRoutesState<C>) -> Router {
    let mut router = Router::new().route("/api/tutor/config", get(tutor_config::<C>));
    if state.enabled {
        router = router.route("/api/tutor/chat", post(tutor_chat::<C>));
    }
    router.with_state(state)
}

/// The coach's coordinates — answers whether the coach is on or off.
#[utoipa::path(
    get,
    path = "/api/tutor/config",
    operation_id = "getTutorConfig",
    responses((status = 200, description = "Whether the coach is on, and its model", body = TutorConfigDto))
)]
pub(crate) async fn tutor_config<C: TutorClient>(
    State(state): State<TutorRoutesState<C>>,
) -> Json<TutorConfigDto> {
    tracing::debug!(enabled = state.enabled, "GET /api/tutor/config");
    Json(TutorConfigDto {
        enabled: state.enabled,
        model: state.model.clone(),
    })
}

/// One coaching turn — the full transcript comes up, one reply goes back.
#[utoipa::path(
    post,
    path = "/api/tutor/chat",
    operation_id = "tutorChat",
    request_body = TutorChatRequestDto,
    responses(
        (status = 200, description = "The coach's reply", body = TutorChatResponseDto),
        (status = 404, description = "The coach is off (the route is never mounted)"),
        (status = 502, description = "Backend failed", body = ApiError),
        (status = 503, description = "Backend unavailable", body = ApiError)
    )
)]
pub(crate) async fn tutor_chat<C: TutorClient>(
    State(state): State<TutorRoutesState<C>>,
    Json(request): Json<TutorChatRequestDto>,
) -> Result<Json<TutorChatResponseDto>, (StatusCode, Json<ApiError>)> {
    tracing::debug!(
        turns = request.messages.len(),
        problem = request.problem_path.as_deref().unwrap_or("-"),
        "POST /api/tutor/chat"
    );
    let context = ChatContext {
        problem_path: request.problem_path,
        code: request.code,
        language: request.language,
    };
    match state.service.chat(&context, &request.messages).await {
        Ok(reply) => Ok(Json(TutorChatResponseDto { content: reply })),
        Err(error) => Err(to_error(&error)),
    }
}

fn to_error(error: &TutorError) -> (StatusCode, Json<ApiError>) {
    match error {
        TutorError::BackendUnavailable(detail) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError {
                error: "Tutor backend unavailable".to_owned(),
                detail: Some(detail.clone()),
                hint: Some("Is a local model server running at TUTOR_URL?".to_owned()),
            }),
        ),
        TutorError::BackendFailed(detail) => (
            StatusCode::BAD_GATEWAY,
            Json(ApiError {
                error: "Tutor backend failed".to_owned(),
                detail: Some(detail.clone()),
                hint: None,
            }),
        ),
    }
}
