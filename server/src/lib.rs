//! The Synapse server, Rust edition — pragmatic hexagonal by bounded context (RS001, mirroring
//! ADR-S007). Each context owns `domain/ application/ infrastructure/ http/` proportional to its
//! complexity; `platform` is the thin, flat cross-cutting context and debuts here with the
//! walking skeleton's one endpoint. `app()` assembles the full HTTP surface; the binary
//! (`main.rs`) is the wiring point.

pub mod config;
pub mod platform;

use axum::Router;
use synapse_shared::api::{ApiError, HealthStatus};
use utoipa::OpenApi;

/// The assembled HTTP surface. Contexts contribute their routers here as they land; integration
/// tests drive this exact router, so what the suite exercises is what the binary serves.
pub fn app() -> Router {
    Router::new().merge(platform::http::routes())
}

/// The code-first OpenAPI document (utoipa). `ApiError` is listed explicitly even though no path
/// here references it yet — it is the shared envelope every context reuses, exactly as the oracle
/// spec keeps it in the generated `Endpoints` (ADR-S012/S019). The contract-lock test diffs this
/// rendered document against `api/openapi.oracle.yaml`.
#[derive(OpenApi)]
#[openapi(
    info(title = "Synapse API", version = "0.1.0"),
    paths(platform::http::get_health),
    components(schemas(HealthStatus, ApiError))
)]
pub struct ApiDoc;
