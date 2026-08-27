//! Binary entry — the wiring point: `main` composes config, logging, and the
//! assembled router; nothing else knows the whole graph. `anyhow` is welcome here and only
//! here — library code carries typed `thiserror` enums per context.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use synapse_server::authoring::application::ProposeEdit;
use synapse_server::authoring::http::AuthoringRoutesState;
use synapse_server::authoring::infrastructure::{
    ConfiguredForges, FsLessonSource, PostgresContentEditors, PostgresEditRequests,
};
use synapse_server::blog::application::BlogService;
use synapse_server::blog::infrastructure::FileSystemBlogRepository;
use synapse_server::catalog::application::CatalogService;
use synapse_server::catalog::application::{Placements, grouping_from_str};
use synapse_server::catalog::domain::content_tree::PRIMARY_SOURCE_ID;
use synapse_server::catalog::domain::merge::Placement;
use synapse_server::catalog::http::admin::ContentSourceRoutesState;
use synapse_server::catalog::infrastructure::{
    ContentCache, ContentSync, FileSystemContentRepository, GitHubFetcher, MountOrder, MountedSources,
    PostgresContentSources, SourceRoot, SyncTrigger, run_content_sync,
};
use synapse_server::execution::application::RunCodeService;
use synapse_server::execution::infrastructure::GoJudgeRunner;
use synapse_server::identity::application::IdentityService;
use synapse_server::identity::http::IdentityRoutesState;
use synapse_server::identity::infrastructure::{JwksTokenVerifier, KeycloakAdminClient};
use synapse_server::platform::rate_limiter::{RateLimitBucket, RateLimiter};
use synapse_server::platform::readiness::PgReadiness;
use synapse_server::progress::PostgresProblemProgress;
use synapse_server::submission::application::SubmitSolution;
use synapse_server::submission::infrastructure::{
    FsProblemTests, PostgresSubmissionAllowlist, PostgresSubmissionRepository, ProgressRecorderAdapter,
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
    // Dev-friendly default: INFO milestones everywhere, DEBUG internals for our own crates;
    // `RUST_LOG` overrides.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,synapse_server=debug,synapse_shared=debug")),
        )
        .init();

    let cfg = synapse_server::config::AppConfig::load()?;

    // Postgres FAILS FAST (Keycloak degrades gracefully instead; the system of record does
    // not); migrations run automatically at boot, on pool acquire.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&cfg.database_url)
        .await?;
    sqlx::migrate!("../migrations").run(&pool).await?;
    tracing::info!("postgres connected + migrations applied");

    // The wiring graph, in one place: config → adapters → services → the router.
    let content = ContentHandles::for_primary(&cfg.content_root, &cfg)?;
    let repo = content.repository(cfg.auto_reload);
    let catalog = Arc::new(CatalogService::with_placements(repo, content.placements.clone()));
    let runner = Arc::new(RunCodeService::new(GoJudgeRunner::new(&cfg.executor_url)));
    let allowlist = Arc::new(PostgresSubmissionAllowlist::new(pool.clone()));
    let views = Arc::new(synapse_server::insights::PostgresLessonViews::new(pool.clone()));
    let readiness = Arc::new(PgReadiness::new(pool.clone()));
    let progress = Arc::new(PostgresProblemProgress::new(pool.clone()));
    let editors = Arc::new(PostgresContentEditors::new(pool.clone()));
    let edit_requests = Arc::new(PostgresEditRequests::new(pool.clone()));
    // Kept back: the submission store takes `pool` by value below, and the source registry is
    // wired later, once identity exists to gate it.
    let content_sources = Arc::new(PostgresContentSources::new(pool.clone()));
    let submit = submit_service(&cfg, pool, &content, &runner, &allowlist, &progress);

    let identity = identity_state(&cfg);
    let limiter = Arc::new(rate_limiter(&cfg));
    let authoring = authoring_state(
        &cfg,
        &identity,
        &limiter,
        editors,
        edit_requests,
        Arc::clone(&content_sources),
        &content,
    );
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
        astro_url = cfg.astro_url().unwrap_or("(api only)"),
        likec4_url = cfg.likec4_url,
        "synapse-rs server started"
    );

    let source_admin = content_registry(
        &cfg,
        &identity,
        Arc::clone(&content_sources),
        Arc::clone(&catalog),
        &content,
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
        progress,
        tutor,
        authoring,
        content_sources: Some(source_admin),
        astro_url: cfg.astro_url().map(str::to_owned),
        d2_render_url: cfg.d2_render_url().map(str::to_owned),
        site_url: cfg.site_url,
        mounted: content.mounted.clone(),
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

/// The two per-caller budgets, anonymous and signed-in. Extracted from `main` for the same reason
/// as its neighbours: the wiring point stays under the per-function line cap.
fn rate_limiter(cfg: &synapse_server::config::AppConfig) -> RateLimiter {
    RateLimiter::new(
        RateLimitBucket {
            window_seconds: cfg.rate_limit_anon_window_seconds,
            limit: cfg.rate_limit_anon_limit,
        },
        RateLimitBucket {
            window_seconds: cfg.rate_limit_auth_window_seconds,
            limit: cfg.rate_limit_auth_limit,
        },
    )
}

/// The two handles every content reader shares. Both are republished by the sync loop as
/// satellites are registered, so they are PASSED AROUND rather than rebuilt: the catalog, the
/// editor's lesson source, the judge's suite lookup and `/media` must all see the same answer, and
/// a second `MountedSources` would silently freeze one of them at boot.
struct ContentHandles {
    /// What the process booted with. Kept whole so the sync loop reconciles additively over it,
    /// rather than re-deriving "what was pinned" from whatever happens to be published by then.
    pinned: MountOrder,
    mounted: MountedSources,
    placements: Placements,
}

impl ContentHandles {
    /// The primary checkout first — always — then any locally-mounted satellites. The sync loop
    /// republishes both lists on its next tick and preserves the same ordering, so a local source
    /// and a fetched one behave identically once mounted.
    fn for_primary(content_root: &str, cfg: &synapse_server::config::AppConfig) -> anyhow::Result<Self> {
        let local = cfg.local_sources()?;
        let mut roots = vec![SourceRoot::new(PRIMARY_SOURCE_ID, content_root)];
        let mut placements = Vec::new();
        mount_local_only(content_root, &mut roots, &mut placements);
        for source in &local {
            tracing::info!(id = %source.id, root = %source.root, grouping = %source.grouping, "mounting a LOCAL content source");
            roots.push(SourceRoot::new(&source.id, &source.root));
            placements.push(Placement {
                source_id: source.id.clone(),
                grouping: grouping_from_str(&source.grouping),
                order: source.order,
            });
        }
        let pinned = MountOrder::pinned(roots, placements);
        let handles = Self {
            mounted: MountedSources::new(pinned.roots().to_vec()),
            placements: Placements::default(),
            pinned,
        };
        handles.placements.publish(handles.pinned.placements().to_vec());
        Ok(handles)
    }

    fn repository(&self, auto_reload: bool) -> FileSystemContentRepository {
        FileSystemContentRepository::mounted(self.mounted.clone(), auto_reload)
    }
}

/// `local-only-content/` mounted as its own source, so its books render at the paths they will
/// have when published rather than under the name of the directory that hides them. AFTER the
/// primary, so the spine wins any contested slug and owns every category declaration.
///
/// A cargo feature rather than an env var, deliberately: an env var would make ADR-RS002's
/// guarantee a deployment-config promise, which is the weakening that ADR was written to avoid.
/// The production binary does not contain this function's body, so no misconfiguration reaches
/// the material. `dev-tools/dev` is the only thing that enables it, and `check-conventions.sh`
/// asserts the image build does not.
#[cfg(feature = "render-local-only")]
fn mount_local_only(content_root: &str, roots: &mut Vec<SourceRoot>, placements: &mut Vec<Placement>) {
    let root = std::path::Path::new(content_root).join("local-only-content");
    if !root.is_dir() {
        return;
    }
    tracing::warn!(
        root = %root.display(),
        "render-local-only: mounting local-only-content — this build must never be deployed"
    );
    roots.push(SourceRoot::new("local-only", &root));
    placements.push(Placement {
        source_id: "local-only".to_owned(),
        grouping: Vec::new(),
        order: None,
    });
}

#[cfg(not(feature = "render-local-only"))]
fn mount_local_only(_content_root: &str, _roots: &mut Vec<SourceRoot>, _placements: &mut Vec<Placement>) {}

/// The reconcile loop owns disk: it fetches registered satellites, unpacks them under the cache,
/// and republishes what is mounted and where each book grafts. Interval `0` disables it, leaving
/// the primary checkout as the whole library — the pre-satellite behaviour, and what the ITs run.
fn spawn_content_sync(
    cfg: &synapse_server::config::AppConfig,
    admin: &ContentSourceRoutesState<PostgresContentSources>,
    content: &ContentHandles,
    trigger: Option<SyncTrigger>,
) {
    let Some(trigger) = trigger else {
        tracing::info!("content sync loop disabled — the primary checkout is the whole library");
        return;
    };
    let sync = ContentSync::new(
        Arc::clone(&admin.sources),
        Arc::new(GitHubFetcher::new(cfg.github_token.clone())),
        ContentCache::new(&cfg.content_cache),
        content.mounted.clone(),
        content.placements.clone(),
        // Pinned: the primary plus any LOCAL satellites. Neither is a registry row, so a reconcile
        // rebuilt from the registry alone would drop them — it has to be additive over these.
        content.pinned.clone(),
    );
    tracing::info!(
        cache = %cfg.content_cache,
        interval_seconds = cfg.content_sync_seconds,
        "content sync loop starting"
    );
    tokio::spawn(run_content_sync(
        sync,
        Duration::from_secs(cfg.content_sync_seconds),
        trigger,
    ));
}

/// The judge: the submission store, the hidden suites resolved through the catalog's own file map,
/// the runner and the allowlist. Extracted for the same reason as its neighbours — `main` is the
/// wiring graph and stays readable as one.
fn submit_service(
    cfg: &synapse_server::config::AppConfig,
    pool: sqlx::PgPool,
    content: &ContentHandles,
    runner: &Arc<synapse_server::execution::http::LiveRunService>,
    allowlist: &Arc<PostgresSubmissionAllowlist>,
    progress: &Arc<PostgresProblemProgress>,
) -> Arc<synapse_server::submission::http::LiveSubmitSolution> {
    Arc::new(SubmitSolution::new(
        Arc::new(PostgresSubmissionRepository::new(pool)),
        Arc::new(FsProblemTests::with_placements(
            content.repository(cfg.auto_reload),
            content.placements.clone(),
        )),
        Arc::clone(runner),
        Arc::clone(allowlist),
        cfg.submission_allowlist_enforced,
        // An accepted submission marks the lesson done in the caller's progress.
        Arc::new(ProgressRecorderAdapter::new(Arc::clone(progress))),
    ))
}

/// The registry's admin surface and the loop that feeds it, joined by one trigger: the loop waits
/// on it, "sync now" notifies it, and it is `None` exactly when the loop is not running — so the
/// route can answer honestly instead of accepting work nobody will do.
fn content_registry(
    cfg: &synapse_server::config::AppConfig,
    identity: &IdentityRoutesState,
    sources: Arc<PostgresContentSources>,
    catalog: Arc<synapse_server::catalog::http::routes::LiveCatalogService>,
    content: &ContentHandles,
) -> ContentSourceRoutesState<PostgresContentSources> {
    let trigger = (cfg.content_sync_seconds > 0).then(SyncTrigger::default);
    let state = content_source_state(sources, identity, catalog, trigger.clone());
    spawn_content_sync(cfg, &state, content, trigger);
    state
}

/// The source registry's admin state — which repositories feed the library. Extracted from `main`
/// for the same reason as its neighbours: the wiring point stays under the per-function line cap.
fn content_source_state(
    sources: Arc<PostgresContentSources>,
    identity: &IdentityRoutesState,
    catalog: Arc<synapse_server::catalog::http::routes::LiveCatalogService>,
    sync: Option<SyncTrigger>,
) -> ContentSourceRoutesState<PostgresContentSources> {
    ContentSourceRoutesState {
        sources,
        identity: Arc::clone(&identity.identity),
        admin_users: Arc::clone(&identity.admin_users),
        catalog,
        sync,
    }
}

/// The identity routes-state: the JWKS token verifier + the Keycloak admin client, plus the
/// issuer/audience/admin-set the routes carry. Extracted from `main` so the wiring point stays
/// scannable (and under the per-function line cap).
fn identity_state(cfg: &synapse_server::config::AppConfig) -> IdentityRoutesState {
    IdentityRoutesState {
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
    }
}

/// The authoring routes-state, or `None` when `CONTENT_FORGE=off` — which leaves the whole
/// `/api/edits` surface and its admin allowlist unmounted rather than gated per request.
///
/// The lesson source gets its OWN `FileSystemContentRepository` rather than sharing the catalog
/// service's: the catalog caches its index per content version, and an editor must read the file
/// as it is on disk this instant, because the fingerprint it hands out is a promise about those
/// exact bytes.
fn authoring_state(
    cfg: &synapse_server::config::AppConfig,
    identity: &IdentityRoutesState,
    limiter: &Arc<synapse_server::platform::rate_limiter::RateLimiter>,
    editors: Arc<PostgresContentEditors>,
    requests: Arc<PostgresEditRequests>,
    sources: Arc<PostgresContentSources>,
    content: &ContentHandles,
) -> Option<AuthoringRoutesState> {
    if cfg.content_forge == "off" {
        tracing::info!("content editing: off — /api/edits is not mounted");
        return None;
    }
    // The forge is chosen PER EDIT, from the registry: a satellite guide repo's lesson opens its
    // pull request against that repository, not the monorepo.
    let forges = ConfiguredForges::new(
        sources,
        &cfg.content_forge,
        &cfg.github_token,
        &cfg.site_url,
        &cfg.content_repo,
        &cfg.content_repo_branch,
    );
    let service = ProposeEdit::new(
        Arc::new(FsLessonSource::with_placements(
            content.repository(cfg.auto_reload),
            content.placements.clone(),
        )),
        Arc::clone(&editors),
        requests,
        Arc::new(forges),
    );
    Some(AuthoringRoutesState {
        service: Arc::new(service),
        identity: Arc::clone(&identity.identity),
        editors,
        admin_users: Arc::clone(&identity.admin_users),
        limiter: Arc::clone(limiter),
    })
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
