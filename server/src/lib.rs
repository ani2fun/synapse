//! The Synapse server, Rust edition — pragmatic hexagonal by bounded context (RS001, mirroring
//! ADR-S007). Each context owns `domain/ application/ infrastructure/ http/` proportional to its
//! complexity; `platform` is the thin, flat cross-cutting context. `app()` assembles the full
//! HTTP surface; the binary (`main.rs`) is the wiring point.

pub mod catalog;
pub mod config;
pub mod platform;

use std::sync::Arc;

use axum::Router;
use catalog::http::LiveCatalogService;
use synapse_shared::api::{ApiError, HealthStatus};
use synapse_shared::catalog::{ComponentDocDto, LessonPayloadDto, SynapseIndexDto};
use utoipa::OpenApi;

/// The assembled HTTP surface. Contexts contribute their routers here as they land; integration
/// tests drive this exact router, so what the suite exercises is what the binary serves.
/// `ContentCacheControl` wraps the whole surface — it stamps only public content GETs on 200.
pub fn app(catalog: Arc<LiveCatalogService>) -> Router {
    Router::new()
        .merge(platform::http::routes())
        .merge(catalog::http::routes(catalog))
        .layer(axum::middleware::from_fn(platform::content_cache_control::stamp))
}

/// The code-first OpenAPI document (utoipa). The contract-lock test diffs this rendered
/// document against `api/openapi.oracle.yaml`; the catalog endpoints are code-first in the
/// oracle too (ADR-S012), so they appear here first and the oracle copy grows when ported
/// endpoints reach it.
#[derive(OpenApi)]
#[openapi(
    info(title = "Synapse API", version = "0.1.0"),
    paths(
        platform::http::get_health,
        catalog::http::routes::get_synapse_index,
        catalog::http::routes::get_component_doc,
        catalog::http::routes::get_synapse_lesson
    ),
    components(schemas(HealthStatus, ApiError, SynapseIndexDto, LessonPayloadDto, ComponentDocDto))
)]
pub struct ApiDoc;
