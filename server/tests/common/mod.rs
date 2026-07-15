//! Shared IT plumbing: the real assembled router over a filesystem repo.

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use synapse_server::catalog::application::CatalogService;
use synapse_server::catalog::infrastructure::FileSystemContentRepository;

/// The full app over a content root (integration tests drive the REAL stack, middleware and
/// all). A nonexistent root is valid — the catalog is simply empty.
pub fn app_over(content_root: &Path) -> Router {
    let repo = FileSystemContentRepository::new(content_root, true);
    synapse_server::app(Arc::new(CatalogService::new(repo)))
}
