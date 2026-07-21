//! The identity HTTP surface (`IdentityRoutes`): the frontend's Keycloak coordinates, the
//! who-am-I echo, and `DELETE /api/me` (the account-deletion endpoint). Deleting the account
//! removes ONLY the sign-in; app data has its own verb, orchestrated by the client.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::identity::{AuthConfigDto, MeDto};

use crate::identity::application::{AuthError, IdentityService};
use crate::identity::infrastructure::{JwksTokenVerifier, KeycloakAdminClient};

pub type LiveIdentityService = IdentityService<JwksTokenVerifier, KeycloakAdminClient>;

#[derive(Clone)]
pub struct IdentityRoutesState {
    pub identity: Arc<LiveIdentityService>,
    pub issuer: String,
    pub audience: String,
    /// Lowercase `ADMIN_USERS` — feeds `MeDto.admin` (UX) and the admin route gate (enforced).
    pub admin_users: Arc<std::collections::HashSet<String>>,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

pub fn routes(state: IdentityRoutesState) -> Router {
    Router::new()
        .route("/api/me", get(get_me).delete(delete_me))
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

/// Resolve the caller when the bearer is OPTIONAL: absent = anonymous (`Ok(None)`); a
/// PRESENT bearer must verify — bad tokens 401, never silently anonymous (the rule every
/// context enforces, stated once here). Callers keep their own anonymous POLICY
/// (allow / 401 / empty list) and their own missing-token copy on top of this skeleton.
pub async fn optional_user(
    identity: &LiveIdentityService,
    headers: &HeaderMap,
) -> Result<Option<crate::identity::domain::AuthenticatedUser>, (StatusCode, Json<ApiError>)> {
    match bearer(headers) {
        None => Ok(None),
        Some(token) => match identity.authenticate(&token).await {
            Ok(user) => Ok(Some(user)),
            Err(error) => Err(to_auth_error(&error)),
        },
    }
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
    let Some(user) = optional_user(&state.identity, &headers).await? else {
        return Err(missing_token());
    };
    let admin = state.admin_users.contains(&user.username);
    Ok(Json(MeDto {
        id: user.id.0,
        username: user.username,
        email: user.email,
        admin, // UX flag only — the admin routes re-check per call
    }))
}

/// Delete the caller's sign-in: verified bearer required; the Keycloak
/// call rides the SCOPED service-account client. Failures are loud — never a swallowed
/// success.
#[utoipa::path(
    delete,
    path = "/api/me",
    operation_id = "deleteMe",
    responses(
        (status = 200, description = "The Keycloak account is gone"),
        (status = 401, description = "Missing or invalid bearer", body = ApiError),
        (status = 503, description = "Keycloak admin API unavailable", body = ApiError)
    )
)]
pub(crate) async fn delete_me(
    State(state): State<IdentityRoutesState>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    let Some(user) = optional_user(&state.identity, &headers).await? else {
        return Err(missing_token());
    };
    match state.identity.delete_account(&user.id.0).await {
        Ok(()) => Ok(Json(serde_json::json!({ "deleted": true }))),
        Err(error) => Err(to_auth_error(&error)),
    }
}

/// The frontend's Keycloak coordinates, split from the issuer.
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
