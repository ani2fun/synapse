//! The catalog use cases — the `ContentRepository` output port the adapters implement, the source
//! registry that says which repositories feed the library, the context's errors, and the service
//! with the version-gated index cache.

mod content_fetcher;
mod content_repository;
mod content_sources;
mod service;

pub use content_fetcher::{ContentFetcher, FetchError, Fetched};
pub use content_repository::{ContentError, ContentRepository};
pub use content_sources::{
    ContentSourceDraft, ContentSourceRecord, ContentSources, Placements, RegistryError, SyncOutcome,
    grouping_from_str, grouping_to_string,
};
pub use service::CatalogService;
