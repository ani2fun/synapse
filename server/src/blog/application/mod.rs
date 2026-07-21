//! The blog use cases (`BlogService` + `BlogError` + the `BlogRepository` port): the
//! listing is version-gated (like the catalog index), post bodies are re-read every call so
//! live edits show, and each post carries its publish-order neighbours.

use std::cmp::Reverse;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::blog::domain::{BlogPost, BlogSummary};

/// What the blog needs from the outside world (same AFIT + generic-service shape as catalog).
pub trait BlogRepository: Send + Sync {
    /// The change watermark — infallible; a degraded filesystem reports a constant.
    fn version(&self) -> impl Future<Output = String> + Send;

    /// Every post as `(slug, raw markdown)`.
    fn load_all(&self) -> impl Future<Output = Result<Vec<(String, String)>, BlogError>> + Send;

    /// One post's raw markdown by slug — traversal-guarded by the adapter.
    fn read(&self, slug: &str) -> impl Future<Output = Result<String, BlogError>> + Send;
}

/// HTTP mapping (at `http/`): `NotFound`→404, `Io`→500.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlogError {
    #[error("no such post: {0}")]
    NotFound(String),
    #[error("blog IO error: {0}")]
    Io(String),
}

/// One post with its publish-order neighbours: `prev` = older, `next` = newer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostView {
    pub post: BlogPost,
    pub prev: Option<String>,
    pub next: Option<String>,
}

pub struct BlogService<R> {
    repo: R,
    /// `(content version, listing)` — rebuilt only when the version moves.
    cache: RwLock<Option<(String, Arc<Vec<BlogSummary>>)>>,
}

impl<R: BlogRepository> BlogService<R> {
    pub fn new(repo: R) -> Self {
        Self {
            repo,
            cache: RwLock::new(None),
        }
    }

    /// The listing, newest first (undated posts sink to the bottom).
    pub async fn list(&self) -> Result<Arc<Vec<BlogSummary>>, BlogError> {
        tracing::debug!("blog: list requested");
        self.summaries().await
    }

    /// One post + neighbours. The body is re-read every call; only the listing is cached.
    pub async fn post(&self, slug: &str) -> Result<BlogPostView, BlogError> {
        let raw = self.repo.read(slug).await.inspect_err(|error| {
            if matches!(error, BlogError::NotFound(_)) {
                tracing::warn!(slug, "blog: post not found");
            }
        })?;
        let summaries = self.summaries().await?;
        let order: Vec<&str> = summaries.iter().map(|s| s.slug.as_str()).collect();
        let i = order.iter().position(|s| *s == slug);
        let prev = i
            .filter(|&i| i + 1 < order.len())
            .map(|i| order[i + 1].to_owned());
        let next = i.filter(|&i| i > 0).map(|i| order[i - 1].to_owned());
        tracing::debug!(slug, ?prev, ?next, "blog: post resolved");
        Ok(BlogPostView {
            post: BlogPost::parse(slug, &raw),
            prev,
            next,
        })
    }

    async fn summaries(&self) -> Result<Arc<Vec<BlogSummary>>, BlogError> {
        let version = self.repo.version().await;
        if let Some((cached_version, listing)) = &*self.cache.read().await
            && *cached_version == version
        {
            tracing::debug!("blog: listing cache hit");
            return Ok(Arc::clone(listing));
        }
        let mut listing: Vec<BlogSummary> = self
            .repo
            .load_all()
            .await?
            .iter()
            .map(|(slug, raw)| BlogPost::parse(slug, raw).summary_view())
            .collect();
        listing.sort_by_key(|s| Reverse(s.published_at));
        tracing::info!(posts = listing.len(), "blog: listing built");
        let listing = Arc::new(listing);
        *self.cache.write().await = Some((version, Arc::clone(&listing)));
        Ok(listing)
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
