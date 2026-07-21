//! The platform-level wire contracts — the walking skeleton's surface. Code-first here:
//! `utoipa` derives the OpenAPI schema from these types, and the contract-lock test
//! (`server/tests/contract_it.rs`) diffs the rendered spec against the committed
//! `api/openapi.oracle.yaml`.

use serde::{Deserialize, Serialize};

/// Walking-skeleton health — just a status string today. Real backing-store checks
/// (Postgres / go-judge / Keycloak) join it when those stores are actually wired; the spec
/// grows to match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct HealthStatus {
    #[cfg_attr(feature = "openapi", schema(example = "ok"))]
    pub status: String,
}

/// The shared error envelope, reused by every context's endpoints. `detail` carries a longer
/// message; `hint` (optional) is operator-facing (e.g. an env var to set).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ApiError {
    #[cfg_attr(feature = "openapi", schema(example = "Not found"))]
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[cfg_attr(feature = "openapi", schema(example = "Set SYNAPSE_ROOT"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}
