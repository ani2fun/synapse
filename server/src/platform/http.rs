//! Inbound HTTP adapter for the `platform` context: axum routes → use cases. Every endpoint
//! carries a layered trace from route down to service, so a request's path through the system
//! is visible in the logs. DTO↔domain mapping lives only here — trivially so for health, whose
//! result *is* the shared DTO.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use synapse_shared::api::HealthStatus;

use crate::platform::health::{self, ReadinessProbe};

/// The context's route table, merged into the app router by `lib.rs`.
pub fn routes(readiness: Arc<dyn ReadinessProbe>) -> Router {
    Router::new()
        .route("/api/health", get(get_health))
        .route("/api/ready", get(get_ready))
        .with_state(readiness)
}

/// Liveness check — 200 while the server is up. Point the orchestrator's `livenessProbe` HERE,
/// never at `/api/ready`: this one must keep answering while a dependency wobbles, or a restart
/// storm replaces a recoverable outage with an unrecoverable one.
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

/// Readiness check — 200 when the backing store answers, 503 when it does not, so the
/// orchestrator stops routing traffic to an instance that cannot serve it. The failure reason
/// goes to the log ONLY; the body stays generic because store errors name hosts and usernames.
#[utoipa::path(
    get,
    path = "/api/ready",
    operation_id = "getReady",
    responses(
        (status = 200, description = "Ready to serve", body = HealthStatus),
        (status = 503, description = "A backing store is unreachable", body = HealthStatus),
    )
)]
pub(crate) async fn get_ready(
    State(probe): State<Arc<dyn ReadinessProbe>>,
) -> (StatusCode, Json<HealthStatus>) {
    tracing::debug!("GET /api/ready");
    match probe.ping().await {
        Ok(()) => (
            StatusCode::OK,
            Json(HealthStatus {
                status: "ready".to_owned(),
            }),
        ),
        Err(detail) => {
            tracing::warn!(%detail, "readiness: the backing store did not answer");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthStatus {
                    status: "not ready".to_owned(),
                }),
            )
        }
    }
}
