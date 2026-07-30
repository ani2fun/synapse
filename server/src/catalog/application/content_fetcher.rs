//! Getting a satellite's content onto disk.
//!
//! Satellites cannot use the git-sync sidecar the primary checkout rides: sidecars are declared in
//! the deploy manifest, and the whole point of the registry is that adding a repository is a row,
//! not a redeploy. So the server fetches over the forge's REST API instead — the same "no git
//! binary, no working copy" stance the authoring forge already takes (ADR-RS004).

/// What one fetch attempt found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    /// The branch head still matches the commit already on disk. The overwhelmingly common
    /// outcome, and the reason the head check is a separate, cheap call.
    Unchanged,
    /// A new commit, with the archive to unpack.
    Archive { sha: String, bytes: Vec<u8> },
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
    /// Seconds until the quota resets. NOTHING ACTS ON IT: `ContentSync::fail` records every
    /// variant the same way, so the next tick refetches immediately and stays throttled. Backing
    /// off is the correct response and is not implemented — the field is here, the branch is not.
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
