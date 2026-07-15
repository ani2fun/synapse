//! Binary entry — the wiring point (RS001's DIP rule: `main` composes config, logging, and the
//! assembled router; nothing else knows the whole graph). `anyhow` is welcome here and only
//! here — library code carries typed `thiserror` enums per context.

use std::net::SocketAddr;
use std::sync::Arc;

use synapse_server::catalog::application::CatalogService;
use synapse_server::catalog::infrastructure::FileSystemContentRepository;
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

    // The wiring graph, in one place: config → adapters → services → the router.
    let repo = FileSystemContentRepository::new(&cfg.content_root, cfg.auto_reload);
    let catalog = Arc::new(CatalogService::new(repo));

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(
        port = cfg.port,
        content_root = cfg.content_root,
        auto_reload = cfg.auto_reload,
        "synapse-rs server started"
    );

    axum::serve(listener, synapse_server::app(catalog)).await?;
    Ok(())
}
