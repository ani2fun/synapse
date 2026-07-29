//! The catalog's outbound adapters — the filesystem repository over the mounted checkouts, the
//! git-SHA content version, and the Postgres source registry.

mod commit_sha;
mod content_cache;
mod filesystem;
mod github_fetcher;
mod mount_order;
mod postgres;
mod sync;

pub use commit_sha::read_commit_sha;
pub use content_cache::ContentCache;
pub use filesystem::{FileSystemContentRepository, MountedSources, SourceRoot};
pub use github_fetcher::GitHubFetcher;
pub use mount_order::MountOrder;
pub use postgres::PostgresContentSources;
pub use sync::{ContentSync, DEFAULT_INTERVAL, SyncTrigger, run as run_content_sync};
