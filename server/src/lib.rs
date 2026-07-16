//! The Synapse server, Rust edition — pragmatic hexagonal by bounded context (RS001, mirroring
//! ADR-S007). Each context owns `domain/ application/ infrastructure/ http/` proportional to its
//! complexity; `platform` is the thin, flat cross-cutting context. `app()` assembles the full
//! HTTP surface; the binary (`main.rs`) is the wiring point.

pub mod catalog;
pub mod config;
pub mod execution;
pub mod identity;
pub mod platform;
pub mod submission;

use std::sync::Arc;

use axum::Router;
use catalog::http::LiveCatalogService;
use execution::http::LiveRunService;
use identity::http::IdentityRoutesState;
use submission::http::{LiveSubmitSolution, SubmissionRoutesState};
use synapse_shared::api::{ApiError, HealthStatus};
use synapse_shared::catalog::{ComponentDocDto, LessonPayloadDto, SynapseIndexDto};
use synapse_shared::execution::{RunRequest, RunResult};
use synapse_shared::identity::{AuthConfigDto, MeDto};
use synapse_shared::submission::{DeleteResultDto, SubmissionAcceptedDto, SubmissionDto, SubmitRequestDto};
use utoipa::OpenApi;

/// The assembled HTTP surface. Contexts contribute their routers here as they land; integration
/// tests drive this exact router, so what the suite exercises is what the binary serves.
/// `ContentCacheControl` wraps the whole surface — it stamps only public content GETs on 200.
pub fn app(
    catalog: Arc<LiveCatalogService>,
    run: Arc<LiveRunService>,
    submit: Arc<LiveSubmitSolution>,
    ident: IdentityRoutesState,
) -> Router {
    let submissions = SubmissionRoutesState {
        submit,
        identity: Arc::clone(&ident.identity),
    };
    Router::new()
        .merge(platform::http::routes())
        .merge(catalog::http::routes(catalog))
        .merge(execution::http::routes(run))
        .merge(submission::http::routes(submissions))
        .merge(identity::http::routes(ident))
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
        catalog::http::routes::get_synapse_lesson,
        execution::http::run_code,
        submission::http::submit_solution,
        submission::http::get_submission,
        submission::http::list_submissions,
        submission::http::delete_submission,
        submission::http::erase_all,
        identity::http::get_me,
        identity::http::get_auth_config
    ),
    components(schemas(
        HealthStatus,
        ApiError,
        SynapseIndexDto,
        LessonPayloadDto,
        ComponentDocDto,
        RunRequest,
        RunResult,
        SubmitRequestDto,
        SubmissionAcceptedDto,
        SubmissionDto,
        DeleteResultDto,
        MeDto,
        AuthConfigDto
    ))
)]
pub struct ApiDoc;
