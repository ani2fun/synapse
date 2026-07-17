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
/// stay green; the static root is absent so no SPA routes mount.
#[allow(dead_code)] // each IT binary compiles common on its own; not all use every helper
pub fn deps(content_root: &Path) -> AppDeps {
    deps_with(
        content_root,
        "http://127.0.0.1:9",
        None,
        "http://127.0.0.1:9/realms/synapse",
    )
}

/// The knobs every IT combination needs: executor, database, issuer.
pub fn deps_with(
    content_root: &Path,
    executor_url: &str,
    pool: Option<sqlx::PgPool>,
    issuer: &str,
) -> AppDeps {
    let pool = pool.unwrap_or_else(|| {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody:nowhere@127.0.0.1:9/none")
            .unwrap_or_else(|e| unreachable!("lazy pools do not connect: {e}"))
    });
    let repo = FileSystemContentRepository::new(content_root, true);
    let runner = Arc::new(RunCodeService::new(GoJudgeRunner::new(executor_url)));
    let allowlist = Arc::new(PostgresSubmissionAllowlist::new(pool.clone()));
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
        static_root: content_root.join("__no_dist__").to_string_lossy().into_owned(),
        content_root: content_root.to_string_lossy().into_owned(),
        likec4_url: "http://127.0.0.1:9".to_owned(),
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
