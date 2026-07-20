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
use synapse_server::platform::readiness::PgReadiness;
use synapse_server::submission::application::SubmitSolution;
use synapse_server::submission::infrastructure::{
    FsProblemTests, PostgresSubmissionAllowlist, PostgresSubmissionRepository,
};
use synapse_server::tutoring::application::TutoringService;
use synapse_server::tutoring::http::TutorRoutesState;
use synapse_server::tutoring::infrastructure::OllamaTutorClient;
use tracing_subscriber::EnvFilter;

/// How long a submission may sit unfinished before a restart declares it dead. Comfortably
/// above the judge's worst case (go-judge caps a run at 100s, suites are small), so the sweep
/// can never steal a run that is genuinely still going.
const JUDGE_GRACE: chrono::Duration = chrono::Duration::minutes(15);

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
    let views = Arc::new(synapse_server::insights::PostgresLessonViews::new(pool.clone()));
    let readiness = Arc::new(PgReadiness::new(pool.clone()));
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

    // Reconcile before serving: a previous process may have died mid-judge, and its rows would
    // otherwise stay unfinished forever (the in-task backstop went down with it). The grace
    // window must clear the slowest realistic suite so a restart never fails a run another
    // replica is still working on.
    if let Err(error) = submit.reconcile_unfinished(JUDGE_GRACE).await {
        // Degraded, not fatal: stale rows are a nuisance, an unservable site is an outage.
        tracing::warn!(%error, "could not reconcile unfinished submissions at boot");
    }

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
        views,
        tutor,
        astro_url: cfg.astro_url,
        site_url: cfg.site_url,
        static_root: cfg.static_root.clone(),
        content_root: cfg.content_root.clone(),
        likec4_url: cfg.likec4_url.clone(),
        readiness,
    });
    // Connect info feeds the anonymous rate-limit key's socket-peer fallback.
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("drained — bye");
    Ok(())
}

/// Resolves on SIGTERM (what Kubernetes sends first on a rolling update or eviction) or on
/// Ctrl-C (the dev loop). Without this, `axum::serve` runs until the process is killed and
/// in-flight requests die mid-response; the pod's `terminationGracePeriodSeconds` must exceed
/// the drain this allows. It does NOT save detached judging tasks — those are covered by the
/// boot-time reconciler, which is the durable half of the fix (OOM kills get no signal at all).
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            // Never resolve rather than take the process down over a missing handler.
            Err(error) => {
                tracing::warn!(%error, "no SIGTERM handler — falling back to ctrl-c only");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("ctrl-c received — draining"),
        () = terminate => tracing::info!("SIGTERM received — draining"),
    }
}
