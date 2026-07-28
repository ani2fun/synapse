//! The sync loop: keep disk, and the mounted source list, in step with the registry.
//!
//! One tick reconciles three things that drift independently — what is registered (Postgres), what
//! is on disk (the cache), and what the catalog is serving (`MountedSources`). It runs on the same
//! 60-second cadence git-sync uses for the primary checkout, which is what
//! `platform::content_cache_control`'s `max-age` is already tuned to.
//!
//! Two rules shape everything here:
//!
//! - **A failing source must never take the library down.** Every failure is recorded on its row
//!   and the tick moves on; a source that has never landed is simply an absent book, and one whose
//!   fetch broke keeps serving the commit it already has.
//! - **The primary checkout is always mounted, and always first.** It is not in the registry, it
//!   cannot be unregistered, and its position is what decides the merge's first-wins rule — so a
//!   book being migrated out of the monorepo keeps serving from there until it is deleted there.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use crate::catalog::application::{
    ContentFetcher, ContentSourceRecord, ContentSources, FetchError, Fetched, Placements, SyncOutcome,
};
use crate::catalog::domain::merge::Placement;
use crate::catalog::infrastructure::content_cache::ContentCache;
use crate::catalog::infrastructure::filesystem::{MountedSources, SourceRoot};

pub const DEFAULT_INTERVAL: Duration = Duration::from_mins(1);

/// Wakes the loop early. Registering a repository or fixing a typo in its grouping should show up
/// now, not on the next tick — a minute of staring at a stale row is how someone concludes the
/// feature is broken and starts editing the database by hand.
pub type SyncTrigger = Arc<tokio::sync::Notify>;

pub struct ContentSync<R, F> {
    registry: Arc<R>,
    fetcher: Arc<F>,
    cache: ContentCache,
    mounted: MountedSources,
    placements: Placements,
    /// Mounted on every tick regardless of the registry, in this order and ahead of everything
    /// fetched: the git-sync'd monorepo first, then any locally-mounted satellites. They are not
    /// registry rows, so a reconcile that rebuilt the mount from the registry alone would drop
    /// them — reconciling has to be additive over what the process was started with.
    pinned: Vec<SourceRoot>,
    pinned_placements: Vec<Placement>,
}

impl<R: ContentSources, F: ContentFetcher> ContentSync<R, F> {
    pub fn new(
        registry: Arc<R>,
        fetcher: Arc<F>,
        cache: ContentCache,
        mounted: MountedSources,
        placements: Placements,
        pinned: Vec<SourceRoot>,
        pinned_placements: Vec<Placement>,
    ) -> Self {
        Self {
            registry,
            fetcher,
            cache,
            mounted,
            placements,
            pinned,
            pinned_placements,
        }
    }

    /// Reconcile once. Returns the number of sources whose content moved, for the caller's log.
    pub async fn tick(&self) -> usize {
        let registered = match self.registry.list().await {
            Ok(rows) => rows,
            Err(error) => {
                // The registry being unreachable must not unmount what is already serving.
                tracing::warn!(%error, "content sync: registry unavailable, keeping the current mount");
                return 0;
            }
        };

        let mut landed = 0;
        let mut roots = self.pinned.clone();
        let mut placements = self.pinned_placements.clone();
        let mut known = BTreeSet::new();

        for source in registered.iter().filter(|s| s.enabled) {
            known.insert(source.id.clone());
            if self.sync_one(source).await {
                landed += 1;
            }
            // Mounted whether or not THIS tick moved it: a source with a good checkout and a
            // failed refresh must keep serving.
            roots.push(SourceRoot::new(
                source.id.clone(),
                self.cache.checkout_of(&source.id),
            ));
            placements.push(source.placement());
        }

        // A disabled or removed source keeps its row's history but stops occupying disk.
        for source in registered.iter().filter(|s| !s.enabled) {
            known.insert(source.id.clone());
        }
        self.reclaim_unregistered(&known);

        self.mounted.publish(roots);
        self.placements.publish(placements);
        landed
    }

    /// Returns whether new content landed.
    async fn sync_one(&self, source: &ContentSourceRecord) -> bool {
        let known = source.last_sha.as_deref();
        // A row that claims a commit but has no checkout — a fresh pod on an empty cache volume —
        // must refetch, or the book would stay absent until someone happened to push.
        let known = if self.cache.checkout_of(&source.id).exists() {
            known
        } else {
            None
        };

        match self.fetcher.fetch(&source.repo, &source.branch, known).await {
            Ok(Fetched::Unchanged) => false,
            Ok(Fetched::Archive { sha, bytes }) => match self.cache.publish(&source.id, &sha, &bytes) {
                Ok(_) => {
                    tracing::info!(id = %source.id, repo = %source.repo, %sha, "content source updated");
                    self.record(&source.id, &SyncOutcome::Landed(sha)).await;
                    true
                }
                Err(error) => {
                    self.fail(&source.id, &error).await;
                    false
                }
            },
            Err(error) => {
                self.fail(&source.id, &error).await;
                false
            }
        }
    }

    async fn fail(&self, id: &str, error: &FetchError) {
        tracing::warn!(id, %error, "content source sync failed — serving the last good checkout");
        self.record(id, &SyncOutcome::Failed(error.to_string())).await;
    }

    async fn record(&self, id: &str, outcome: &SyncOutcome) {
        if let Err(error) = self.registry.record_sync(id, outcome).await {
            tracing::warn!(id, %error, "content sync: could not record the outcome");
        }
    }

    /// Reclaim cache directories for sources nobody registers any more. Deliberately after the
    /// mount is computed: losing disk is recoverable, unmounting a live book is not.
    fn reclaim_unregistered(&self, known: &BTreeSet<String>) {
        for stale in self.cache.cached_source_ids() {
            if !known.contains(&stale) {
                tracing::info!(id = %stale, "content source no longer registered — reclaiming its cache");
                self.cache.forget(&stale);
            }
        }
    }
}

/// Run forever, one reconcile per interval — or sooner, when `trigger` is notified. The first tick
/// happens immediately so a boot does not wait a full period before satellites appear.
pub async fn run<R: ContentSources + 'static, F: ContentFetcher + 'static>(
    sync: ContentSync<R, F>,
    interval: Duration,
    trigger: SyncTrigger,
) {
    loop {
        let landed = sync.tick().await;
        if landed > 0 {
            tracing::info!(landed, "content sync: sources updated");
        }
        // A notify that arrives DURING a tick is not lost: `Notify` stores one permit, so the
        // wait below returns immediately and the next reconcile sees the change that prompted it.
        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            () = trigger.notified() => tracing::info!("content sync: reconcile requested"),
        }
    }
}
