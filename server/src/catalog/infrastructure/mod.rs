//! The catalog's outbound adapters — the filesystem repository over `SYNAPSE_ROOT` and the
//! git-SHA content version.

mod commit_sha;
mod filesystem;

pub use commit_sha::read_commit_sha;
pub use filesystem::FileSystemContentRepository;
