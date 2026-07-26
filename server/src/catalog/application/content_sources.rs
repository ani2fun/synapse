//! The source registry: which repositories feed the library, where their books sit, and how the
//! last fetch went.
//!
//! This is the seam that makes a satellite repo a row rather than a redeploy. The primary checkout
//! is wired in code and never appears here — it arrives by git-sync, is always mounted, and is
//! always first, which is what makes the first-wins merge rule safe during a migration.

use chrono::{DateTime, Utc};

use crate::catalog::application::content_repository::ContentError;
use crate::catalog::domain::merge::Placement;
use crate::catalog::domain::walker::{slug_like, slugify};

/// A registered repository, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSourceRecord {
    /// Slug-shaped, immutable: lesson file references point back through it and it names the
    /// source's cache directory on disk.
    pub id: String,
    /// `owner/name`.
    pub repo: String,
    pub branch: String,
    /// Category slug path the book grafts under; empty is the top level.
    pub grouping: Vec<String>,
    /// Overrides `book.json`'s own `order` when set.
    pub order: Option<i32>,
    pub enabled: bool,
    pub last_sha: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl ContentSourceRecord {
    /// Where this source's book lands, for the merge.
    #[must_use]
    pub fn placement(&self) -> Placement {
        Placement {
            source_id: self.id.clone(),
            grouping: self.grouping.clone(),
            order: self.order,
        }
    }
}

/// What the admin panel supplies. The id is derived, not typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSourceDraft {
    pub repo: String,
    pub branch: String,
    pub grouping: Vec<String>,
    pub order: Option<i32>,
    pub enabled: bool,
}

impl ContentSourceDraft {
    /// `ani2fun/java-guide` → `java-guide`. Deterministic, so the cache directory and every stored
    /// file reference stay stable across restarts.
    #[must_use]
    pub fn derive_id(repo: &str) -> String {
        slugify(repo.rsplit('/').next().unwrap_or(repo))
    }

    /// Reject what the catalog cannot serve, at the door rather than at render time. The grouping
    /// matters most: it is not slug-checked anywhere downstream, and it reaches `<loc>` in
    /// `/sitemap.xml` by way of the book's category path.
    pub fn validate(&self) -> Result<String, RegistryError> {
        let (owner, name) = self
            .repo
            .split_once('/')
            .ok_or_else(|| RegistryError::Invalid("repo must be owner/name".to_owned()))?;
        if owner.trim().is_empty() || name.trim().is_empty() || name.contains('/') {
            return Err(RegistryError::Invalid("repo must be owner/name".to_owned()));
        }
        if self.branch.trim().is_empty() {
            return Err(RegistryError::Invalid("branch must not be blank".to_owned()));
        }
        if !self.grouping.iter().all(|segment| slug_like(segment)) {
            return Err(RegistryError::Invalid(format!(
                "grouping segments must be slug-like: '{}'",
                self.grouping.join("/")
            )));
        }
        let id = Self::derive_id(&self.repo);
        if !slug_like(&id) {
            return Err(RegistryError::Invalid(format!(
                "'{}' yields no usable source id",
                self.repo
            )));
        }
        Ok(id)
    }
}

/// How a fetch attempt ended, for the row's sync columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    /// The checkout now holds this commit.
    Landed(String),
    /// Nothing landed. The previous `last_sha` is deliberately kept: stale content beats none.
    Failed(String),
}

/// The registry's output port.
pub trait ContentSources: Send + Sync {
    /// Every registered source, enabled first, in mount order.
    fn list(&self) -> impl Future<Output = Result<Vec<ContentSourceRecord>, RegistryError>> + Send;

    /// Register or re-register a repository, keyed on its derived id.
    fn upsert(
        &self,
        draft: &ContentSourceDraft,
    ) -> impl Future<Output = Result<ContentSourceRecord, RegistryError>> + Send;

    /// Forget a repository. Its cached checkout is reclaimed by the fetch loop.
    fn remove(&self, id: &str) -> impl Future<Output = Result<bool, RegistryError>> + Send;

    /// Record how the last fetch went.
    fn record_sync(
        &self,
        id: &str,
        outcome: &SyncOutcome,
    ) -> impl Future<Output = Result<(), RegistryError>> + Send;
}

/// The registry's error. `Invalid` is the caller's fault (400); `StoreFailed` is ours (500).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("invalid content source: {0}")]
    Invalid(String),
    #[error("content source store error: {0}")]
    StoreFailed(String),
}

impl From<RegistryError> for ContentError {
    fn from(error: RegistryError) -> Self {
        Self::Io(error.to_string())
    }
}

/// Where each registered source's book grafts, as the catalog currently believes it.
///
/// A runtime cache of the registry, republished whole by the sync loop so a reader never sees a
/// half-updated set. It is shared rather than re-queried because EVERY lesson resolution needs
/// it: a satellite's URL includes its grouping, so resolving without placements would look the
/// book up at the wrong path.
#[derive(Clone, Default)]
pub struct Placements {
    inner: std::sync::Arc<std::sync::RwLock<Vec<Placement>>>,
}

impl Placements {
    /// A poisoned lock degrades to "no placements" rather than panicking a request: the writer is
    /// a background loop, and one failed publish must not take reads down.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Placement> {
        self.inner.read().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn publish(&self, placements: Vec<Placement>) {
        if let Ok(mut held) = self.inner.write() {
            *held = placements;
        }
    }
}

/// `""` ⇒ the top level; `"a/b"` ⇒ nested. Blank segments are dropped so a stray slash cannot
/// produce an unnameable category.
#[must_use]
pub fn grouping_from_str(raw: &str) -> Vec<String> {
    raw.split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[must_use]
pub fn grouping_to_string(grouping: &[String]) -> String {
    grouping.join("/")
}

#[cfg(test)]
#[path = "content_sources_tests.rs"]
mod tests;
