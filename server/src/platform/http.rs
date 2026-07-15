//! Inbound HTTP adapter for the `platform` context: axum routes → use cases, with the layered
//! trace (route → service) every endpoint carries (ADR-S009). DTO↔domain mapping lives only
//! here — trivially so for health, whose result *is* the shared DTO.

use axum::routing::get;
use axum::{Json, Router};
use synapse_shared::api::HealthStatus;

use crate::platform::health;

/// The context's route table, merged into the app router by `lib.rs`.
pub fn routes() -> Router {
    Router::new().route("/api/health", get(get_health))
}

/// Liveness check — 200 while the server is up.
#[utoipa::path(
    get,
    path = "/api/health",
    operation_id = "getHealth",
    responses((status = 200, description = "OK", body = HealthStatus))
)]
pub(crate) async fn get_health() -> Json<HealthStatus> {
    tracing::info!("GET /api/health");
    Json(health::status())
}
