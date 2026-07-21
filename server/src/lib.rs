//! The Synapse server — pragmatic hexagonal by bounded context. Each context owns
//! `domain/ application/ infrastructure/ http/` proportional to its
//! complexity; `platform` is the thin, flat cross-cutting context. `app()` assembles the full
//! HTTP surface; the binary (`main.rs`) is the wiring point.

pub mod blog;
pub mod catalog;
pub mod config;
pub mod execution;
pub mod identity;
pub mod insights;
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
use submission::http::{LiveSubmitSolution, SubmissionRoutesState};
use synapse_shared::api::{ApiError, HealthStatus};
use synapse_shared::blog::{BlogPostDto, BlogSummaryDto};
use synapse_shared::catalog::{BookEntryDto, ChapterDto, ComponentDocDto, LessonPayloadDto, SynapseIndexDto};
use synapse_shared::execution::{RunRequest, RunResult};
use synapse_shared::identity::{AuthConfigDto, MeDto};
use synapse_shared::insights::LessonViewDto;
use synapse_shared::submission::{
    AllowlistEntryDto, DeleteResultDto, GrantRequestDto, SubmissionAcceptedDto, SubmissionDto,
    SubmitRequestDto,
};
use utoipa::OpenApi;

/// Everything `app` composes — one wiring struct so `main` and the ITs build the same graph
/// field by field.
///
/// Generic (dependency inversion at the wiring boundary) over the three ports a test wants to fake
/// through the FULL router: the allowlist, the lesson-view store, and the tutor client. The
/// defaults are the production adapters, so `main` and the common IT helper spell nothing
/// extra; an IT that passes a fake gets the whole `app()` — layer stack included — instead
/// of assembling its own sub-router. `submit` stays concrete on purpose: its `List` param is
/// the Postgres allowlist, and parameterizing the four-param service through here would be
/// generics sprawl for no current test need (the admin router is what the fakes exercise).
pub struct AppDeps<
    L = submission::infrastructure::PostgresSubmissionAllowlist,
    V = insights::PostgresLessonViews,
    C = tutoring::infrastructure::OllamaTutorClient,
> where
    L: submission::application::SubmissionAllowlist + 'static,
    V: insights::LessonViewStore + 'static,
    C: tutoring::application::TutorClient + 'static,
{
    pub catalog: Arc<LiveCatalogService>,
    pub run: Arc<LiveRunService>,
    pub submit: Arc<LiveSubmitSolution>,
    pub ident: IdentityRoutesState,
    pub blog: Arc<LiveBlogService>,
    pub limiter: Arc<RateLimiter>,
    /// The allowlist store the admin panel manages (the submit gate holds its own Arc).
    pub allowlist: Arc<L>,
    /// Readership: the catalog records into it, the admin panel reads it.
    pub views: Arc<V>,
    /// The Astro SSR sidecar serving the pages. `Some` mounts `astro_proxy` as the router
    /// FALLBACK (registered routes always win); `None` (dev without a web tier) serves the
    /// API alone with a plain-text pointer at `/`.
    pub astro_url: Option<String>,
    /// Public origin for the sitemap's absolute URLs.
    pub site_url: String,
    /// The content checkout — `/media` serves its `_media/` tree (one shared cache hour).
    pub content_root: String,
    pub likec4_url: String,
    /// Answers `/api/ready`: Postgres in the binary, the same lazy pool in ITs (which then
    /// report 503 — the honest answer for a store that is not there).
    pub readiness: Arc<dyn platform::health::ReadinessProbe>,
    /// The coach: when disabled the chat route is never mounted — a structural 404.
    pub tutor: tutoring::http::TutorRoutesState<C>,
}

/// The assembled HTTP surface. Contexts contribute their routers here; integration tests drive
/// this exact router, so what the suite exercises is what the binary serves. Precedence: API
/// (cache-stamped) → `/media` → `/c4` proxy → robots/sitemap → the Astro page proxy as the
/// FALLBACK, so a registered route can never be shadowed by a page path.
pub fn app<L, V, C>(deps: AppDeps<L, V, C>) -> Router
where
    L: submission::application::SubmissionAllowlist + 'static,
    V: insights::LessonViewStore + 'static,
    C: tutoring::application::TutorClient + 'static,
{
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
    // Crawler plumbing mounts UNCONDITIONALLY, before the page proxy: robots + sitemap are
    // generated from the in-memory catalog, which lives in THIS process.
    let seo = platform::seo_routes::SeoRoutesState {
        catalog: Arc::clone(&deps.catalog),
        site_url: deps.site_url.clone(),
    };
    let media = platform::media_routes::MediaRoutes::new(&deps.content_root);
    let security = platform::security_headers::SecurityHeaders::new(&deps.ident.issuer);
    let admin = submission::http::admin::AdminRoutesState {
        allowlist: deps.allowlist,
        identity: Arc::clone(&deps.ident.identity),
        admin_users: Arc::clone(&deps.ident.admin_users),
    };
    let readership = insights::http::InsightsRoutesState {
        views: Arc::clone(&deps.views),
        identity: Arc::clone(&deps.ident.identity),
        admin_users: Arc::clone(&deps.ident.admin_users),
    };
    let catalog_state = catalog::http::routes::CatalogRoutesState {
        service: deps.catalog,
        views: deps.views,
    };
    let mut router = Router::new()
        .merge(platform::http::routes(deps.readiness))
        .merge(catalog::http::routes(catalog_state))
        .merge(execution::http::routes(execution))
        .merge(submission::http::routes(submissions))
        .merge(identity::http::routes(deps.ident))
        .merge(blog::http::routes(deps.blog))
        .merge(submission::http::admin::routes(admin))
        .merge(insights::http::routes(readership))
        .merge(tutoring::http::routes(deps.tutor))
        .layer(axum::middleware::from_fn(platform::content_cache_control::stamp))
        .merge(media.routes())
        .merge(platform::likec4_proxy::routes(&deps.likec4_url))
        .merge(platform::seo_routes::routes(seo));
    if let Some(astro_url) = deps.astro_url.as_deref() {
        // The page front door: a FALLBACK, so every registered route above keeps winning and
        // the sidecar's 404 page becomes the site 404.
        let proxy = platform::astro_proxy::AstroProxy::new(astro_url);
        router = router.fallback(axum::routing::any(platform::astro_proxy::handle).with_state(proxy));
    } else {
        router = router.route(
            "/",
            get(|| async {
                "synapse server — API only (set SYNAPSE_ASTRO_URL for pages); see /api/health"
            }),
        );
    }
    // Outermost OF THE APPLICATION SUB-TREES: the security stamp covers every route class —
    // API, proxy, static, and error responses alike. The TRANSPORT layers wrap further out
    // still: compression (gzip/deflate at the ORIGIN — a CDN edge-compressing still pulls fat
    // bytes across the tunnel — with sub-KiB responses left alone), then limits, then
    // telemetry as the true outermost layer.
    let stamped = router
        .layer(axum::middleware::from_fn_with_state(
            security,
            platform::security_headers::stamp,
        ))
        .layer(
            tower_http::compression::CompressionLayer::new()
                .gzip(true)
                .deflate(true)
                .compress_when(tower_http::compression::predicate::SizeAbove::new(1024)),
        );
    // Edge limits sit INSIDE tracing and outside everything else: a request killed by the
    // timeout should still produce a span saying so, but nothing below should get the chance
    // to read an unbounded body first.
    let bounded = platform::limits::apply(stamped);
    // Tracing wraps everything, outside even compression: a span that starts
    // inside the header layers cannot report on them, and a request rejected at the edge is
    // exactly the one worth having a trace for.
    platform::telemetry::apply(bounded)
}

/// The code-first OpenAPI document (utoipa). The contract-lock test diffs this rendered
/// document against `api/openapi.oracle.yaml`, the committed reference copy of the API
/// contract; endpoints are code-first, so they appear here first and the reference copy
/// grows as endpoints are added.
#[derive(OpenApi)]
#[openapi(
    info(title = "Synapse API", version = "0.1.0"),
    paths(
        platform::http::get_health,
        platform::http::get_ready,
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
        insights::http::list_lesson_views,
        tutoring::http::tutor_config,
        tutoring::http::tutor_chat
    ),
    components(schemas(
        HealthStatus,
        ApiError,
        SynapseIndexDto,
        // `BookDto.entries` and `ChapterDto.entries` carry `schema(no_recursion)` (they are
        // genuinely self-referential trees) — that stops utoipa's auto-walk from EVER reaching
        // `BookEntryDto`/`ChapterDto`, so without listing them here the rendered document has a
        // dangling `$ref` no `cargo test` catches (the contract-lock test only checks schemas
        // named in the committed reference copy, and it does not name these).
        // `openapi-typescript` is a stricter reader than our own tests — it surfaced this
        // generating `schema.gen.ts`.
        BookEntryDto,
        ChapterDto,
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
        LessonViewDto,
        synapse_shared::tutor::ChatMessage,
        synapse_shared::tutor::TutorConfigDto,
        synapse_shared::tutor::TutorChatRequestDto,
        synapse_shared::tutor::TutorChatResponseDto
    ))
)]
pub struct ApiDoc;
