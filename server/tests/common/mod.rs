//! Shared IT plumbing: the real assembled router over a filesystem repo.

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use synapse_server::AppDeps;
use synapse_server::blog::application::BlogService;
use synapse_server::blog::infrastructure::FileSystemBlogRepository;
use synapse_server::catalog::application::CatalogService;
use synapse_server::catalog::infrastructure::FileSystemContentRepository;
use synapse_server::execution::application::RunCodeService;
use synapse_server::execution::infrastructure::GoJudgeRunner;
use synapse_server::identity::application::IdentityService;
use synapse_server::identity::http::IdentityRoutesState;
use synapse_server::identity::infrastructure::{JwksTokenVerifier, KeycloakAdminClient};
use synapse_server::platform::rate_limiter::{RateLimitBucket, RateLimiter};
use synapse_server::submission::application::SubmitSolution;
use synapse_server::submission::infrastructure::{
    FsProblemTests, PostgresSubmissionAllowlist, PostgresSubmissionRepository,
};
use synapse_server::tutoring::application::TutoringService;
use synapse_server::tutoring::http::TutorRoutesState;
use synapse_server::tutoring::infrastructure::OllamaTutorClient;

/// A budget big enough that only the dedicated rate-limit ITs ever hit it.
const TEST_BUCKET: RateLimitBucket = RateLimitBucket {
    window_seconds: 60,
    limit: 10_000,
};

/// The default wiring over a content root — tests tweak fields before `synapse_server::app`.
/// A nonexistent root is valid (empty catalog + blog); port 9 (discard) refuses connections,
/// so executor/issuer/likec4 default to unreachable; the pool is LAZY so store-free routes
/// stay green; `astro_url` is absent so `/` falls back to the plain API-only pointer text.
#[allow(dead_code)] // each IT binary compiles common on its own; not all use every helper
pub fn deps(content_root: &Path) -> AppDeps {
    deps_with(
        content_root,
        "http://127.0.0.1:9",
        None,
        "http://127.0.0.1:9/realms/synapse",
    )
}

/// A pool that will never connect (port 9 = discard) — store-backed routes answer honestly
/// (503 / empty) without a database.
fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://nobody:nowhere@127.0.0.1:9/none")
        .unwrap_or_else(|e| unreachable!("lazy pools do not connect: {e}"))
}

/// The lazy-pool default stores + the disabled tutor, for ITs that fake only ONE of the
/// three swappable ports and want the production shape for the rest.
#[allow(dead_code)]
pub fn lazy_allowlist() -> Arc<PostgresSubmissionAllowlist> {
    Arc::new(PostgresSubmissionAllowlist::new(lazy_pool()))
}
#[allow(dead_code)]
pub fn lazy_views() -> Arc<synapse_server::insights::PostgresLessonViews> {
    Arc::new(synapse_server::insights::PostgresLessonViews::new(lazy_pool()))
}
#[allow(dead_code)]
pub fn tutor_off() -> TutorRoutesState<OllamaTutorClient> {
    TutorRoutesState {
        service: Arc::new(TutoringService::new(OllamaTutorClient::new(
            "http://127.0.0.1:9",
            "llama3.1",
        ))),
        enabled: false,
        model: "llama3.1".to_owned(),
    }
}

/// The FULL app with caller-supplied stores for the three fakeable ports: an IT that fakes
/// one port passes the defaults above for the others and still drives the whole router —
/// layer stack included — instead of assembling its own sub-router.
#[allow(dead_code)]
pub fn app_with_stores<L, V, C>(
    issuer: &str,
    allowlist: Arc<L>,
    views: Arc<V>,
    tutor: TutorRoutesState<C>,
) -> Router
where
    L: synapse_server::submission::application::SubmissionAllowlist + 'static,
    V: synapse_server::insights::LessonViewStore + 'static,
    C: synapse_server::tutoring::application::TutorClient + 'static,
{
    let base = deps_with(Path::new("__no_content__"), "http://127.0.0.1:9", None, issuer);
    synapse_server::app(AppDeps {
        allowlist,
        views,
        tutor,
        catalog: base.catalog,
        run: base.run,
        submit: base.submit,
        ident: base.ident,
        blog: base.blog,
        limiter: base.limiter,
        astro_url: base.astro_url,
        site_url: base.site_url,
        content_root: base.content_root,
        likec4_url: base.likec4_url,
        readiness: base.readiness,
    })
}

/// The knobs every IT combination needs: executor, database, issuer.
pub fn deps_with(
    content_root: &Path,
    executor_url: &str,
    pool: Option<sqlx::PgPool>,
    issuer: &str,
) -> AppDeps {
    let pool = pool.unwrap_or_else(lazy_pool);
    let repo = FileSystemContentRepository::new(content_root, true);
    let runner = Arc::new(RunCodeService::new(GoJudgeRunner::new(executor_url)));
    let allowlist = Arc::new(PostgresSubmissionAllowlist::new(pool.clone()));
    let views = Arc::new(synapse_server::insights::PostgresLessonViews::new(pool.clone()));
    let readiness = Arc::new(synapse_server::platform::readiness::PgReadiness::new(
        pool.clone(),
    ));
    // Gate OFF (the dev default) — the gate tests exercise it over in-memory fakes.
    let submit = Arc::new(SubmitSolution::new(
        Arc::new(PostgresSubmissionRepository::new(pool)),
        Arc::new(FsProblemTests::new(FileSystemContentRepository::new(
            content_root,
            true,
        ))),
        Arc::clone(&runner),
        Arc::clone(&allowlist),
        false,
    ));
    let ident = IdentityRoutesState {
        identity: Arc::new(IdentityService::new(
            JwksTokenVerifier::new(issuer, "synapse-web"),
            KeycloakAdminClient::new(issuer, "synapse-admin", "dev-admin-secret"),
        )),
        issuer: issuer.to_owned(),
        audience: "synapse-web".to_owned(),
        // The dev default ("tester") — the minted IT token IS tester, so admin ITs pass the gate.
        admin_users: Arc::new(std::collections::HashSet::from(["tester".to_owned()])),
    };
    AppDeps {
        catalog: Arc::new(CatalogService::new(repo)),
        run: runner,
        submit,
        ident,
        allowlist,
        views,
        // The dev default: coach OFF — chat is a structural 404 (the tutor ITs build their own).
        tutor: TutorRoutesState {
            service: Arc::new(TutoringService::new(OllamaTutorClient::new(
                "http://127.0.0.1:9",
                "llama3.1",
            ))),
            enabled: false,
            model: "llama3.1".to_owned(),
        },
        blog: Arc::new(BlogService::new(FileSystemBlogRepository::new(
            content_root,
            true,
        ))),
        limiter: Arc::new(RateLimiter::new(TEST_BUCKET, TEST_BUCKET)),
        astro_url: None,
        site_url: "https://synapse.test".to_owned(),
        content_root: content_root.to_string_lossy().into_owned(),
        likec4_url: "http://127.0.0.1:9".to_owned(),
        readiness,
    }
}

/// The full app over a content root (integration tests drive the REAL stack, middleware and
/// all).
#[allow(dead_code)]
pub fn app_over(content_root: &Path) -> Router {
    synapse_server::app(deps(content_root))
}

/// The full app with an explicit go-judge base URL.
#[allow(dead_code)]
pub fn app_with_executor(content_root: &Path, executor_url: &str) -> Router {
    synapse_server::app(deps_with(
        content_root,
        executor_url,
        None,
        "http://127.0.0.1:9/realms/synapse",
    ))
}

/// The full app with an explicit database too (the gated Postgres ITs).
#[allow(dead_code)]
pub fn app_with(content_root: &Path, executor_url: &str, pool: Option<sqlx::PgPool>) -> Router {
    synapse_server::app(deps_with(
        content_root,
        executor_url,
        pool,
        "http://127.0.0.1:9/realms/synapse",
    ))
}

/// The full app with an explicit OIDC issuer (the identity ITs run a local JWKS stub).
#[allow(dead_code)]
pub fn app_with_issuer(
    content_root: &Path,
    executor_url: &str,
    pool: Option<sqlx::PgPool>,
    issuer: &str,
) -> Router {
    synapse_server::app(deps_with(content_root, executor_url, pool, issuer))
}
