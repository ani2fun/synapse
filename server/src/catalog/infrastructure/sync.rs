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
//!   fetch broke keeps serving the commit it already has. One failure also decides when to try
//!   again: a rate limit says how long there is no point, and [`Throttle`] holds the source until
//!   then rather than spending a tick — and a request off the recovering quota — proving it.
//! - **The primary checkout is always mounted, and always first.** It is not in the registry, it
//!   cannot be unregistered, and its position is what decides the merge's first-wins rule — so a
//!   book being migrated out of the monorepo keeps serving from there until it is deleted there.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::Instant;

use crate::catalog::application::{
    ContentFetcher, ContentSourceRecord, ContentSources, FetchError, Fetched, Placements, SyncOutcome,
};
use crate::catalog::infrastructure::content_cache::ContentCache;
use crate::catalog::infrastructure::filesystem::{MountedSources, SourceRoot};
use crate::catalog::infrastructure::mount_order::MountOrder;

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
    /// What the process booted with — the git-sync'd monorepo and any locally-mounted satellites.
    /// They are not registry rows, so a reconcile rebuilt from the registry alone would drop them:
    /// reconciling is additive over this set, and `MountOrder` is what keeps it in front.
    pinned: MountOrder,
    /// Which sources are waiting out a rate limit. Internal state rather than a collaborator: it
    /// is derived entirely from failures this loop has already seen.
    throttle: Throttle,
}

impl<R: ContentSources, F: ContentFetcher> ContentSync<R, F> {
    pub fn new(
        registry: Arc<R>,
        fetcher: Arc<F>,
        cache: ContentCache,
        mounted: MountedSources,
        placements: Placements,
        pinned: MountOrder,
    ) -> Self {
        Self {
            registry,
            fetcher,
            cache,
            mounted,
            placements,
            pinned,
            throttle: Throttle::default(),
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
        // Every tick starts from the booted set, so the primary checkout leads this list by
        // construction rather than by the order the pushes below happen to run in.
        let mut order = self.pinned.pinned_only();
        let mut known = BTreeSet::new();

        for source in registered.iter().filter(|s| s.enabled) {
            known.insert(source.id.clone());
            if self.sync_one(source).await {
                landed += 1;
            }
            // Mounted whether or not THIS tick moved it: a source with a good checkout and a
            // failed refresh must keep serving.
            order.append(
                SourceRoot::new(source.id.clone(), self.cache.checkout_of(&source.id)),
                source.placement(),
            );
        }

        // A disabled or removed source keeps its row's history but stops occupying disk.
        for source in registered.iter().filter(|s| !s.enabled) {
            known.insert(source.id.clone());
        }
        self.reclaim_unregistered(&known);

        let (roots, placements) = order.into_parts();
        self.mounted.publish(roots);
        self.placements.publish(placements);
        landed
    }

    /// Returns whether new content landed.
    async fn sync_one(&self, source: &ContentSourceRecord) -> bool {
        if let Some(left) = self.throttle.remaining(&source.id) {
            tracing::debug!(
                id = %source.id,
                seconds = left.as_secs(),
                "content source is rate limited — skipping this tick"
            );
            return false;
        }

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
                    let backoff = self.fail(&source.id, &error).await;
                    self.throttle.apply(&source.id, backoff);
                    false
                }
            },
            Err(error) => {
                let backoff = self.fail(&source.id, &error).await;
                self.throttle.apply(&source.id, backoff);
                false
            }
        }
    }

    /// Record how the attempt ended, and answer what it means for the NEXT tick.
    ///
    /// The answer is a [`Backoff`] rather than nothing because this is the one place a
    /// `FetchError`'s payload is read instead of stringified: `RateLimited` knows something no
    /// other variant does — how long there is no point trying — and losing that to
    /// `error.to_string()` here is the flattening ADR-RS001's amendment on error payloads warns
    /// about. The sentence still reaches the row; the deadline reaches the loop.
    async fn fail(&self, id: &str, error: &FetchError) -> Backoff {
        let backoff = if let FetchError::RateLimited { seconds } = error {
            tracing::warn!(
                id,
                seconds,
                "content source rate limited — holding off until the quota resets"
            );
            Backoff::Wait(Duration::from_secs(*seconds))
        } else {
            tracing::warn!(id, %error, "content source sync failed — serving the last good checkout");
            Backoff::NextTick
        };
        self.record(id, &SyncOutcome::Failed(error.to_string())).await;
        backoff
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

/// What a failed fetch means for the next tick.
#[derive(Debug, Clone, Copy)]
enum Backoff {
    /// Try again on the usual cadence. A missing repository, a refused archive, a flaky
    /// transport: the next tick may well succeed, and asking costs one cheap head request.
    NextTick,
    /// The forge's quota is spent for this long. Every fetch until then is a request that cannot
    /// succeed — and that the forge counts anyway.
    ///
    /// A zero window is not special-cased: it means the reset instant has already passed, so the
    /// next tick fetching is the right answer rather than a missing back-off.
    Wait(Duration),
}

/// When each throttled source may be fetched again.
///
/// In memory rather than on the registry row. The loop is one process, so nothing else needs to
/// read this; and `last_error` is a sentence written for an admin, so recovering a deadline would
/// mean parsing one back out of prose — the same flattening, just in the other direction. The cost
/// of holding it here is that a restart forgets: each throttled source spends one request
/// discovering the limit again, which is the price of a deploy rather than of every tick.
#[derive(Default)]
struct Throttle {
    /// Source id → the instant its quota resets. Absent means "fetch freely".
    until: Mutex<HashMap<String, Instant>>,
}

impl Throttle {
    /// How long this source must still wait, or `None` if it may be fetched now. Expired holds are
    /// dropped as they are read, so the map only ever holds sources actually waiting.
    fn remaining(&self, id: &str) -> Option<Duration> {
        let mut until = self.until.lock().ok()?;
        let left = until.get(id)?.saturating_duration_since(Instant::now());
        if left.is_zero() {
            until.remove(id);
            return None;
        }
        Some(left)
    }

    /// Act on a failure's verdict. `NextTick` is a no-op rather than a clear: a source only reaches
    /// [`ContentSync::fail`] when this said it could fetch, so there is no stale hold to lift.
    ///
    /// A poisoned lock degrades to no back-off — the pre-existing behaviour — rather than
    /// panicking the loop that keeps every other source updating.
    fn apply(&self, id: &str, backoff: Backoff) {
        let Backoff::Wait(window) = backoff else {
            return;
        };
        if let Ok(mut until) = self.until.lock() {
            until.insert(id.to_owned(), Instant::now() + window);
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
