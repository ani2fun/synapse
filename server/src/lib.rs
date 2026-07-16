//! The Synapse server, Rust edition — pragmatic hexagonal by bounded context (RS001, mirroring
//! ADR-S007). Each context owns `domain/ application/ infrastructure/ http/` proportional to its
//! complexity; `platform` is the thin, flat cross-cutting context. `app()` assembles the full
//! HTTP surface; the binary (`main.rs`) is the wiring point.

pub mod blog;
pub mod catalog;
pub mod config;
pub mod execution;
pub mod identity;
pub mod platform;
pub mod submission;
pub mod tutoring;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use blog::http::LiveBlogService;
use catalog::http::LiveCatalogService;
use execution::http::{ExecutionRoutesState, LiveRunService};
use identity::http::IdentityRoutesState;
use platform::rate_limiter::RateLimiter;
use platform::static_routes::StaticRoutes;
use submission::http::{LiveSubmitSolution, SubmissionRoutesState};
use synapse_shared::api::{ApiError, HealthStatus};
use synapse_shared::blog::{BlogPostDto, BlogSummaryDto};
use synapse_shared::catalog::{ComponentDocDto, LessonPayloadDto, SynapseIndexDto};
use synapse_shared::execution::{RunRequest, RunResult};
use synapse_shared::identity::{AuthConfigDto, MeDto};
use synapse_shared::submission::{
    AllowlistEntryDto, DeleteResultDto, GrantRequestDto, SubmissionAcceptedDto, SubmissionDto,
    SubmitRequestDto,
};
use utoipa::OpenApi;

/// Everything `app` composes — one wiring struct so `main` and the ITs build the same graph
/// field by field.
pub struct AppDeps {
    pub catalog: Arc<LiveCatalogService>,
    pub run: Arc<LiveRunService>,
    pub submit: Arc<LiveSubmitSolution>,
    pub ident: IdentityRoutesState,
    pub blog: Arc<LiveBlogService>,
    pub limiter: Arc<RateLimiter>,
    /// The allowlist store the admin panel manages (the submit gate holds its own Arc).
    pub allowlist: Arc<submission::infrastructure::PostgresSubmissionAllowlist>,
    /// The production dist dir; absent (dev) → no static routes, and `/` answers plain text.
    pub static_root: String,
    pub likec4_url: String,
    /// The coach (step 22): when disabled the chat route is never mounted — a structural 404.
    pub tutor: tutoring::http::TutorRoutesState<tutoring::infrastructure::OllamaTutorClient>,
}

/// The assembled HTTP surface. Contexts contribute their routers here as they land; integration
/// tests drive this exact router, so what the suite exercises is what the binary serves.
/// Precedence mirrors the oracle: API (cache-stamped) → `/c4` proxy → static+SPA fallback →
/// the plain-text root. The SPA fallback ENUMERATES its segments, so it can never shadow
/// `/api`; `ContentCacheControl` stamps only public content GETs on 200.
pub fn app(deps: AppDeps) -> Router {
    let submissions = SubmissionRoutesState {
        submit: deps.submit,
        identity: Arc::clone(&deps.ident.identity),
        limiter: Arc::clone(&deps.limiter),
    };
    let execution = ExecutionRoutesState {
        run: deps.run,
        identity: Arc::clone(&deps.ident.identity),
        limiter: deps.limiter,
    };
    let statics = StaticRoutes::new(&deps.static_root);
    let security = platform::security_headers::SecurityHeaders::new(&deps.ident.issuer);
    let admin = submission::http::admin::AdminRoutesState {
        allowlist: deps.allowlist,
        identity: Arc::clone(&deps.ident.identity),
        admin_users: Arc::clone(&deps.ident.admin_users),
    };
    let mut router = Router::new()
        .merge(platform::http::routes())
        .merge(catalog::http::routes(deps.catalog))
        .merge(execution::http::routes(execution))
        .merge(submission::http::routes(submissions))
        .merge(identity::http::routes(deps.ident))
        .merge(blog::http::routes(deps.blog))
        .merge(submission::http::admin::routes(admin))
        .merge(tutoring::http::routes(deps.tutor))
        .layer(axum::middleware::from_fn(platform::content_cache_control::stamp))
        .merge(platform::likec4_proxy::routes(&deps.likec4_url));
    if statics.enabled() {
        router = router.merge(statics.routes());
    } else {
        router = router.route(
            "/",
            get(|| async { "synapse-rs server — see /api/health or /api/synapse/index" }),
        );
    }
    // OUTERMOST (step 19): the security stamp covers every sub-tree — API, proxy, static,
    // and error responses alike.
    router.layer(axum::middleware::from_fn_with_state(
        security,
        platform::security_headers::stamp,
    ))
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
        identity::http::get_auth_config,
        identity::http::delete_me,
        blog::http::list_posts,
        blog::http::get_post,
        submission::http::admin::list_allowlist,
        submission::http::admin::grant_allowlist,
        submission::http::admin::revoke_allowlist,
        tutoring::http::tutor_config,
        tutoring::http::tutor_chat
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
        AuthConfigDto,
        BlogSummaryDto,
        BlogPostDto,
        AllowlistEntryDto,
        GrantRequestDto,
        synapse_shared::tutor::ChatMessage,
        synapse_shared::tutor::TutorConfigDto,
        synapse_shared::tutor::TutorChatRequestDto,
        synapse_shared::tutor::TutorChatResponseDto
    ))
)]
pub struct ApiDoc;
