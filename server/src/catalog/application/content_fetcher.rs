//! Getting a satellite's content onto disk.
//!
//! Satellites cannot use the git-sync sidecar the primary checkout rides: sidecars are declared in
//! the deploy manifest, and the whole point of the registry is that adding a repository is a row,
//! not a redeploy. So the server fetches over the forge's REST API instead — the same "no git
//! binary, no working copy" stance the authoring forge already takes (ADR-RS004).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// What one fetch attempt found.
#[derive(Debug, PartialEq, Eq)]
pub enum Fetched {
    /// The branch head still matches the commit already on disk. The overwhelmingly common
    /// outcome, and the reason the head check is a separate, cheap call.
    Unchanged,
    /// A new commit, with the archive to unpack.
    Archive { sha: String, archive: SpooledArchive },
}

/// A fetched archive, on disk, deleted when this is dropped.
///
/// On disk rather than in a `Vec` because an archive's size is the repository's to decide, not
/// this process's: a book that carries its own images runs to hundreds of MB, and the app
/// container's memory limit is 256 MiB. Owning the file makes cleanup structural — every path out
/// of a fetch or an unpack, the error paths included, drops it.
#[derive(Debug, PartialEq, Eq)]
pub struct SpooledArchive {
    path: PathBuf,
}

impl SpooledArchive {
    /// Take ownership of a file at `path`. Adopt BEFORE writing to it, so a failed write is
    /// cleaned up too.
    #[must_use]
    pub fn adopt(path: PathBuf) -> Self {
        Self { path }
    }

    /// Write `bytes` to a fresh file in `dir`. For fakes and tests, which have bytes in hand.
    pub fn from_bytes(dir: &Path, bytes: &[u8]) -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!(
            ".spool-{}-{}.tar.gz",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let archive = Self::adopt(dir.join(name));
        std::fs::write(archive.path(), bytes)?;
        Ok(archive)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SpooledArchive {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The output port. One method, because the two-call protocol (head, then archive) is the
/// adapter's business — a caller that had to sequence it could forget the cheap check.
pub trait ContentFetcher: Send + Sync {
    fn fetch(
        &self,
        repo: &str,
        branch: &str,
        known_sha: Option<&str>,
    ) -> impl Future<Output = Result<Fetched, FetchError>> + Send;
}

/// Why a fetch did not produce content. Every variant lands in the source row's `last_error`, so
/// the wording is what an admin reads when a book stops updating.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FetchError {
    #[error("repository or branch not found: {0}")]
    NotFound(String),
    #[error("access denied: {0}")]
    Denied(String),
    /// Seconds until the quota resets. This is the variant the sync loop BRANCHES on rather than
    /// only recording: `ContentSync::fail` turns it into a hold, and the source is skipped
    /// entirely until the window has passed. Without that, every 60-second tick spends a request
    /// on a source that cannot answer — off the very quota that is trying to refill.
    #[error("rate limited, resets in {seconds}s")]
    RateLimited { seconds: u64 },
    /// The archive exceeded the size or entry cap. Refused loudly rather than unpacked partially.
    #[error("archive refused: {0}")]
    TooLarge(String),
    #[error("unreadable archive: {0}")]
    Malformed(String),
    #[error("transport error: {0}")]
    Transport(String),
}
