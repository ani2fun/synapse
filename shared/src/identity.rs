//! The identity wire contract (oracle: `shared/identity/IdentityApi.scala`).

use serde::{Deserialize, Serialize};

/// The verified caller (`GET /api/me`). `admin` is UX-only — the server re-checks per call
/// (it joins with the admin step; false until then).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MeDto {
    pub id: String,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub admin: bool,
}

/// The SPA's Keycloak coordinates (`GET /api/auth/config`) — exactly
/// `new Keycloak({url, realm, clientId})`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AuthConfigDto {
    pub url: String,
    pub realm: String,
    pub client_id: String,
}
