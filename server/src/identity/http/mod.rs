//! The identity HTTP surface (oracle: `IdentityRoutes`, step-17 scope): the SPA's Keycloak
//! coordinates and the who-am-I echo. `DELETE /api/me` joins with the account step.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::identity::{AuthConfigDto, MeDto};

use crate::identity::application::{AuthError, IdentityService};
use crate::identity::infrastructure::JwksTokenVerifier;

pub type LiveIdentityService = IdentityService<JwksTokenVerifier>;

#[derive(Clone)]
pub struct IdentityRoutesState {
    pub identity: Arc<LiveIdentityService>,
    pub issuer: String,
    pub audience: String,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

pub fn routes(state: IdentityRoutesState) -> Router {
    Router::new()
        .route("/api/me", get(get_me))
        .route("/api/auth/config", get(get_auth_config))
        .with_state(state)
}

/// The bearer, if the caller sent one.
pub fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned)
}

/// `InvalidToken`→401 · `VerifierUnavailable`→503 — every consumer re-states this mapping.
pub fn to_auth_error(error: &AuthError) -> (StatusCode, Json<ApiError>) {
    let (status, message) = match error {
        AuthError::InvalidToken(_) => (StatusCode::UNAUTHORIZED, "Invalid bearer token"),
        AuthError::VerifierUnavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "Token verifier unavailable"),
    };
    (
        status,
        Json(ApiError {
            error: message.to_owned(),
            detail: Some(error.to_string()),
            hint: None,
        }),
    )
}

fn missing_token() -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(ApiError {
            error: "Missing bearer token".to_owned(),
            detail: Some("Send Authorization: Bearer <token>".to_owned()),
            hint: None,
        }),
    )
}

/// Who am I — the verified caller's echo.
#[utoipa::path(
    get,
    path = "/api/me",
    operation_id = "getMe",
    responses(
        (status = 200, description = "The verified caller", body = MeDto),
        (status = 401, description = "Missing or invalid bearer", body = ApiError),
        (status = 503, description = "Verifier unavailable", body = ApiError)
    )
)]
pub(crate) async fn get_me(State(state): State<IdentityRoutesState>, headers: HeaderMap) -> ApiResult<MeDto> {
    let Some(token) = bearer(&headers) else {
        return Err(missing_token());
    };
    match state.identity.authenticate(&token).await {
        Ok(user) => Ok(Json(MeDto {
            id: user.id.0,
            username: user.username,
            email: user.email,
            admin: false, // UX flag — joins with the admin step; the server re-checks anyway
        })),
        Err(error) => Err(to_auth_error(&error)),
    }
}

/// The SPA's Keycloak coordinates, split from the issuer.
#[utoipa::path(
    get,
    path = "/api/auth/config",
    operation_id = "getAuthConfig",
    responses(
        (status = 200, description = "Keycloak coordinates", body = AuthConfigDto),
        (status = 500, description = "The issuer is not a Keycloak realm URL", body = ApiError)
    )
)]
pub(crate) async fn get_auth_config(State(state): State<IdentityRoutesState>) -> ApiResult<AuthConfigDto> {
    let issuer = state.issuer.trim_end_matches('/');
    match issuer.split_once("/realms/") {
        Some((url, realm)) if !url.is_empty() && !realm.is_empty() => Ok(Json(AuthConfigDto {
            url: url.to_owned(),
            realm: realm.to_owned(),
            client_id: state.audience.clone(),
        })),
        _ => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: "Identity issuer is not a Keycloak realm URL".to_owned(),
                detail: Some(format!("issuer: {issuer}")),
                hint: Some("Set OIDC_ISSUER to http(s)://…/realms/<realm>".to_owned()),
            }),
        )),
    }
}
