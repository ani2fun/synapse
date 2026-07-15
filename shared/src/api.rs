//! The platform-level wire contracts — the walking skeleton's surface (oracle:
//! `api/openapi.yaml` + `synapse.shared.api.Endpoints`, ADR-S019). Code-first here: `utoipa`
//! derives the OpenAPI schema from these types, and the contract-lock test
//! (`server/tests/contract_it.rs`) diffs the rendered spec against the committed oracle spec.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Walking-skeleton health — just a status string today. Real backing-store checks
/// (Postgres / go-judge / Keycloak) join it when those stores are actually wired; the spec
/// grows to match (ADR-S019).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HealthStatus {
    #[schema(example = "ok")]
    pub status: String,
}

/// The shared error envelope, reused by every context's endpoints. `detail` carries a longer
/// message; `hint` (optional) is operator-facing (e.g. an env var to set).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    #[schema(example = "Not found")]
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[schema(example = "Set SYNAPSE_ROOT")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}
