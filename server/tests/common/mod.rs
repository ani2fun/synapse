//! Shared IT plumbing: the real assembled router over a filesystem repo.

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use synapse_server::catalog::application::CatalogService;
use synapse_server::catalog::infrastructure::FileSystemContentRepository;
use synapse_server::execution::application::RunCodeService;
use synapse_server::execution::infrastructure::GoJudgeRunner;
use synapse_server::identity::application::IdentityService;
use synapse_server::identity::http::IdentityRoutesState;
use synapse_server::identity::infrastructure::JwksTokenVerifier;
use synapse_server::submission::application::SubmitSolution;
use synapse_server::submission::infrastructure::{FsProblemTests, PostgresSubmissionRepository};

/// The full app over a content root (integration tests drive the REAL stack, middleware and
/// all). A nonexistent root is valid — the catalog is simply empty.
#[allow(dead_code)] // each IT binary compiles common on its own; not all use every helper
pub fn app_over(content_root: &Path) -> Router {
    // Port 9 (discard) refuses connections — tests that need a live sandbox point the
    // executor elsewhere via `app_with_executor`.
    app_with(content_root, "http://127.0.0.1:9", None)
}

/// The full app with an explicit go-judge base URL.
#[allow(dead_code)] // each IT binary compiles common on its own; not all use every helper
pub fn app_with_executor(content_root: &Path, executor_url: &str) -> Router {
    app_with(content_root, executor_url, None)
}

/// The full app with an explicit database too (the gated Postgres ITs). Without one, a LAZY
/// pool pointed at a refusing port stands in — routes that never touch the store stay green.
pub fn app_with(content_root: &Path, executor_url: &str, pool: Option<sqlx::PgPool>) -> Router {
    // A refusing issuer: anonymous paths work; token paths 503 (Keycloak-down semantics).
    app_with_issuer(
        content_root,
        executor_url,
        pool,
        "http://127.0.0.1:9/realms/synapse",
    )
}

/// The full app with an explicit OIDC issuer (the identity ITs run a local JWKS stub).
#[allow(dead_code)]
pub fn app_with_issuer(
    content_root: &Path,
    executor_url: &str,
    pool: Option<sqlx::PgPool>,
    issuer: &str,
) -> Router {
    let pool = pool.unwrap_or_else(|| {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody:nowhere@127.0.0.1:9/none")
            .unwrap_or_else(|e| unreachable!("lazy pools do not connect: {e}"))
    });
    let repo = FileSystemContentRepository::new(content_root, true);
    let runner = Arc::new(RunCodeService::new(GoJudgeRunner::new(executor_url)));
    let submit = Arc::new(SubmitSolution::new(
        Arc::new(PostgresSubmissionRepository::new(pool)),
        Arc::new(FsProblemTests::new(FileSystemContentRepository::new(
            content_root,
            true,
        ))),
        Arc::clone(&runner),
    ));
    let identity = IdentityRoutesState {
        identity: Arc::new(IdentityService::new(JwksTokenVerifier::new(
            issuer,
            "synapse-web",
        ))),
        issuer: issuer.to_owned(),
        audience: "synapse-web".to_owned(),
    };
    synapse_server::app(Arc::new(CatalogService::new(repo)), runner, submit, identity)
}
