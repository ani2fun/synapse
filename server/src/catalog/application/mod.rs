//! The catalog use cases — the `ContentRepository` output port the adapters implement, the
//! context's error, and the service with the version-gated index cache.

mod content_repository;
mod service;

pub use content_repository::{ContentError, ContentRepository};
pub use service::CatalogService;
