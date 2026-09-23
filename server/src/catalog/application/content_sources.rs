//! The source registry: which repositories feed the library, where their books sit, and how the
//! last fetch went.
//!
//! This is the seam that makes a satellite repo a row rather than a redeploy. The primary checkout
//! is wired in code and never appears here — it arrives by git-sync, is always mounted, and is
//! always first, which is what makes the first-wins merge rule safe during a migration.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};

use crate::catalog::domain::merge::Placement;
use crate::catalog::domain::walker::{slug_like, slugify};
use crate::identity::domain::Username;

/// Who a source is for. Decided by the REGISTRATION, never by the repository: a `book.json` is
/// authored inside the repository and cannot be trusted to declare itself public.
///
/// Fetching does not change with this — a private repository lands through the same token and
/// the same tarball as a public one. What changes is every read path: the index, search, the
/// lesson itself and the sitemap each ask `admits` before answering.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Audience {
    #[default]
    Public,
    /// Served to these names only. Empty is legal and means nobody, which is what a freshly
    /// registered private repository IS until an admin adds the first reader.
    Private { readers: BTreeSet<Username> },
}

impl Audience {
    /// `"public"` / `"private"` on the wire and in the row. Anything else is a caller's error.
    pub fn parse(raw: &str, readers: BTreeSet<Username>) -> Result<Self, RegistryError> {
        match raw.trim() {
            "" | "public" => Ok(Self::Public),
            "private" => Ok(Self::Private { readers }),
            other => Err(RegistryError::Invalid(format!(
                "visibility must be public or private, not '{other}'"
            ))),
        }
    }

    #[must_use]
    pub fn visibility(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private { .. } => "private",
        }
    }

    #[must_use]
    pub fn is_private(&self) -> bool {
        matches!(self, Self::Private { .. })
    }

    /// Whether this viewer may read. Anonymous never reads a private source — there is no name
    /// to find on the list.
    #[must_use]
    pub fn admits(&self, viewer: &Viewer) -> bool {
        match (self, viewer) {
            (Self::Public, _) => true,
            (Self::Private { .. }, Viewer::Anonymous) => false,
            (Self::Private { readers }, Viewer::User(name)) => readers.contains(name),
        }
    }
}

/// Who is asking. Resolved by the HTTP layer only when it matters — a public read never verifies a
/// token, because a JWKS check on every page view is a real cost for a bit that is usually unread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Viewer {
    Anonymous,
    User(Username),
}

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
    pub audience: Audience,
    pub last_sha: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

/// One reader on a private source's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentReader {
    pub username: Username,
    pub note: Option<String>,
    pub granted_at: DateTime<Utc>,
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

/// The branch a registration means when it does not say. Declared here rather than only in the
/// schema: what a registration MEANS is this layer's business, and the SQL default is a backstop.
pub const DEFAULT_BRANCH: &str = "main";

/// A registration with this layer's defaults applied and its rules already enforced.
///
/// [`ContentSourceDraft::register`] is the only door, so an adapter receives nothing it has to
/// re-check. That used to be the adapter's job, and it showed: the Postgres store validated and
/// then re-trimmed every field on the way to SQL, and the route suite's fake registry had to
/// replicate the same `validate` call to behave like the real one. Both are what an invariant
/// looks like when it lives in the adapters instead of the type they share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSourceDraft {
    id: String,
    repo: String,
    branch: String,
    grouping: Vec<String>,
    order: Option<i32>,
    enabled: bool,
    /// `public` or `private`. The reader list is not part of a registration — it is granted and
    /// revoked name by name afterwards, and re-registering a repository must not blank it.
    visibility: &'static str,
}

impl ContentSourceDraft {
    /// `ani2fun/java-guide` → `java-guide`. Deterministic, so the cache directory and every stored
    /// file reference stay stable across restarts.
    #[must_use]
    pub fn derive_id(repo: &str) -> String {
        slugify(repo.rsplit('/').next().unwrap_or(repo))
    }

    /// Apply what a registration means when it is silent, then refuse what the catalog cannot
    /// serve — at the door rather than at render time. The grouping matters most: nothing
    /// downstream slug-checks it, and it reaches `<loc>` in `/sitemap.xml` by way of the book's
    /// category path.
    ///
    /// A blank branch is not an error, it is silence: it means [`DEFAULT_BRANCH`], the same
    /// answer the route used to compute for itself.
    pub fn register(
        repo: &str,
        branch: Option<&str>,
        grouping: Option<&str>,
        order: Option<i32>,
        enabled: Option<bool>,
        visibility: Option<&str>,
    ) -> Result<Self, RegistryError> {
        let repo = repo.trim();
        let visibility = Audience::parse(visibility.unwrap_or_default(), BTreeSet::new())?.visibility();
        let (owner, name) = repo
            .split_once('/')
            .ok_or_else(|| RegistryError::Invalid("repo must be owner/name".to_owned()))?;
        if owner.trim().is_empty() || name.trim().is_empty() || name.contains('/') {
            return Err(RegistryError::Invalid("repo must be owner/name".to_owned()));
        }
        let grouping = grouping_from_str(grouping.unwrap_or_default());
        if !grouping.iter().all(|segment| slug_like(segment)) {
            return Err(RegistryError::Invalid(format!(
                "grouping segments must be slug-like: '{}'",
                grouping.join("/")
            )));
        }
        let id = Self::derive_id(repo);
        if !slug_like(&id) {
            return Err(RegistryError::Invalid(format!(
                "'{repo}' yields no usable source id"
            )));
        }
        Ok(Self {
            id,
            repo: repo.to_owned(),
            branch: branch
                .map(str::trim)
                .filter(|b| !b.is_empty())
                .unwrap_or(DEFAULT_BRANCH)
                .to_owned(),
            grouping,
            order,
            enabled: enabled.unwrap_or(true),
            visibility,
        })
    }

    /// The derived id — already computed and checked at construction, never re-derived downstream.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn repo(&self) -> &str {
        &self.repo
    }
    #[must_use]
    pub fn branch(&self) -> &str {
        &self.branch
    }
    #[must_use]
    pub fn grouping(&self) -> &[String] {
        &self.grouping
    }
    #[must_use]
    pub fn order(&self) -> Option<i32> {
        self.order
    }
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    #[must_use]
    pub fn visibility(&self) -> &'static str {
        self.visibility
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

    /// A source's reader list, newest grant first. `None` when there is no such source.
    fn list_readers(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<Vec<ContentReader>>, RegistryError>> + Send;

    /// Upsert a reader — re-granting refreshes the note. `None` when there is no such source.
    fn grant_reader(
        &self,
        id: &str,
        username: &Username,
        note: Option<&str>,
    ) -> impl Future<Output = Result<Option<ContentReader>, RegistryError>> + Send;

    /// `false` when there was nothing to revoke.
    fn revoke_reader(
        &self,
        id: &str,
        username: &Username,
    ) -> impl Future<Output = Result<bool, RegistryError>> + Send;
}

/// The registry's error. `Invalid` is the caller's fault (400); `StoreFailed` is ours (500).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("invalid content source: {0}")]
    Invalid(String),
    #[error("content source store error: {0}")]
    StoreFailed(String),
}

/// Who may read each mounted source, as the catalog currently believes it.
///
/// A runtime cache like [`Placements`], republished whole by the sync loop — and deliberately NOT
/// part of the version-gated catalog snapshot, which is keyed by CONTENT version: a reader granted
/// at ten must be admitted at ten, not at the next content push. The pinned half is what the
/// process booted with (the primary checkout and any local satellites, which are not registry
/// rows); a publish carries only the registered sources and never displaces a pinned entry, the
/// same rule the mount order keeps.
#[derive(Clone, Default)]
pub struct Audiences {
    pinned: Arc<BTreeMap<String, Audience>>,
    live: Arc<RwLock<BTreeMap<String, Audience>>>,
}

impl Audiences {
    #[must_use]
    pub fn pinned(pinned: BTreeMap<String, Audience>) -> Self {
        Self {
            live: Arc::new(RwLock::new(pinned.clone())),
            pinned: Arc::new(pinned),
        }
    }

    /// Replace the registered half. Pinned sources keep their own answer.
    pub fn publish(&self, registered: BTreeMap<String, Audience>) {
        let mut next = registered;
        for (id, audience) in self.pinned.iter() {
            next.insert(id.clone(), audience.clone());
        }
        if let Ok(mut held) = self.live.write() {
            *held = next;
        }
    }

    /// A source nobody registered an audience for — the primary checkout, `local-only`, anything
    /// unknown — is public, which is what every source was before audiences existed. A poisoned
    /// lock also answers public: the writer is a background loop, and one failed publish must not
    /// lock every reader out.
    #[must_use]
    pub fn of(&self, source_id: &str) -> Audience {
        self.live
            .read()
            .ok()
            .and_then(|held| held.get(source_id).cloned())
            .unwrap_or_default()
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
mod tests;
