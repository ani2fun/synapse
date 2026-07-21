//! The catalog's output port and its error type.

use crate::catalog::domain::catalog::SynapseContentError;
use crate::catalog::domain::content_tree::ContentEntry;

/// What the catalog needs from the outside world. Native async-fn-in-trait + generic services
/// (static dispatch): nothing varies at runtime, so `dyn` would be ceremony (RS001).
pub trait ContentRepository: Send + Sync {
    /// The change watermark: in dev this is an mtime/count watermark so live edits show up
    /// immediately; in prod it is the checkout's git SHA (advances when git-sync moves).
    /// Infallible — degraded filesystems report a constant, they don't fail the request.
    fn content_version(&self) -> impl Future<Output = String> + Send;

    /// The raw tree under the content root, metadata pre-decoded.
    fn load_tree(&self) -> impl Future<Output = Result<Vec<ContentEntry>, ContentError>> + Send;

    /// One file by content-root-relative path (lesson bodies, sidecars) — traversal-guarded by
    /// the adapter, re-read per request so live edits show.
    fn read_lesson(&self, path: &str) -> impl Future<Output = Result<String, ContentError>> + Send;
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
