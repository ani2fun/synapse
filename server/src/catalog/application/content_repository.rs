//! The catalog's output port and its error type.

use crate::catalog::domain::catalog::SynapseContentError;
use crate::catalog::domain::content_tree::SourceTree;

/// What the catalog needs from the outside world. Native async-fn-in-trait + generic services
/// (static dispatch): nothing varies at runtime, so `dyn` would be ceremony (RS001).
pub trait ContentRepository: Send + Sync {
    /// The change watermark across every mounted source: in dev an mtime/count watermark so live
    /// edits show up immediately; in prod each checkout's git SHA (advances when its sync moves).
    /// Infallible — degraded filesystems report a constant, they don't fail the request.
    fn content_version(&self) -> impl Future<Output = String> + Send;

    /// The raw tree of every mounted source, root and subdirectory metadata pre-decoded.
    fn load_sources(&self) -> impl Future<Output = Result<Vec<SourceTree>, ContentError>> + Send;

    /// One file, by the id of the source that owns it and its path within that source's root
    /// (lesson bodies, sidecars) — traversal-guarded by the adapter, re-read per request so live
    /// edits show. An unknown `source_id` is `NotFound`: reads never fall through to another
    /// source, or two books with the same interior layout would cross-serve each other's files.
    fn read_lesson(
        &self,
        source_id: &str,
        path: &str,
    ) -> impl Future<Output = Result<String, ContentError>> + Send;
}

/// The context's error. The HTTP layer maps these to status codes: `NotFound`→404, `Io`→500,
/// `IndexInvalid`→500.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContentError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("catalog IO error: {0}")]
    Io(String),
    #[error("catalog index invalid: {0}")]
    IndexInvalid(SynapseContentError),
}
