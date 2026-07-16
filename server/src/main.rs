//! Binary entry — the wiring point (RS001's DIP rule: `main` composes config, logging, and the
//! assembled router; nothing else knows the whole graph). `anyhow` is welcome here and only
//! here — library code carries typed `thiserror` enums per context.

use std::net::SocketAddr;
use std::sync::Arc;

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
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Dev-friendly default: INFO milestones everywhere, DEBUG internals for our own crates
    // (ADR-S009); `RUST_LOG` overrides.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,synapse_server=debug,synapse_shared=debug")),
        )
        .init();

    let cfg = synapse_server::config::AppConfig::load()?;

    // Postgres FAILS FAST (oracle parity: Keycloak degrades, the system of record does not);
    // migrations run at boot, exactly like Liquibase on pool acquire.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&cfg.database_url)
        .await?;
    sqlx::migrate!("../migrations").run(&pool).await?;
    tracing::info!("postgres connected + migrations applied");

    // The wiring graph, in one place: config → adapters → services → the router.
    let repo = FileSystemContentRepository::new(&cfg.content_root, cfg.auto_reload);
    let catalog = Arc::new(CatalogService::new(repo));
    let runner = Arc::new(RunCodeService::new(GoJudgeRunner::new(&cfg.executor_url)));
    let allowlist = Arc::new(PostgresSubmissionAllowlist::new(pool.clone()));
    let submit = Arc::new(SubmitSolution::new(
        Arc::new(PostgresSubmissionRepository::new(pool)),
        Arc::new(FsProblemTests::new(FileSystemContentRepository::new(
            &cfg.content_root,
            cfg.auto_reload,
        ))),
        Arc::clone(&runner),
        Arc::clone(&allowlist),
        cfg.submission_allowlist_enforced,
    ));

    let identity = IdentityRoutesState {
        identity: Arc::new(IdentityService::new(
            JwksTokenVerifier::new(&cfg.identity_issuer, &cfg.identity_audience),
            KeycloakAdminClient::new(
                &cfg.identity_issuer,
                &cfg.keycloak_admin_client_id,
                &cfg.keycloak_admin_client_secret,
            ),
        )),
        issuer: cfg.identity_issuer.clone(),
        audience: cfg.identity_audience.clone(),
        admin_users: Arc::new(cfg.admin_user_set()),
    };
    let tutor = TutorRoutesState {
        service: Arc::new(TutoringService::new(OllamaTutorClient::new(
            &cfg.tutor_url,
            &cfg.tutor_model,
        ))),
        enabled: cfg.tutor_enabled,
        model: cfg.tutor_model.clone(),
    };
    let blog = Arc::new(BlogService::new(FileSystemBlogRepository::new(
        &cfg.content_root,
        cfg.auto_reload,
    )));
    let limiter = Arc::new(RateLimiter::new(
        RateLimitBucket {
            window_seconds: cfg.rate_limit_anon_window_seconds,
            limit: cfg.rate_limit_anon_limit,
        },
        RateLimitBucket {
            window_seconds: cfg.rate_limit_auth_window_seconds,
            limit: cfg.rate_limit_auth_limit,
        },
    ));

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(
        port = cfg.port,
        content_root = cfg.content_root,
        auto_reload = cfg.auto_reload,
        executor_url = cfg.executor_url,
        static_root = cfg.static_root,
        likec4_url = cfg.likec4_url,
        "synapse-rs server started"
    );

    let app = synapse_server::app(synapse_server::AppDeps {
        catalog,
        run: runner,
        submit,
        ident: identity,
        blog,
        limiter,
        allowlist,
        tutor,
        static_root: cfg.static_root.clone(),
        likec4_url: cfg.likec4_url.clone(),
    });
    // Connect info feeds the anonymous rate-limit key's socket-peer fallback.
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}
