//! Integration: the sync loop end to end — registry → fetch → unpack → mount → serve.
//!
//! The fetcher and the Postgres registry are faked (each has its own suite); everything below
//! them is real: the cache unpacks a genuine gzipped tar, the filesystem repository walks what
//! lands, and the catalog service resolves URLs through it.
//!
//! The scenario this exists for is the CUTOVER — the window where a book lives both in the
//! monorepo and in its own repository. Getting that wrong is silently destructive, because every
//! readership and progress row is keyed on the lesson path.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flate2::Compression;
use flate2::write::GzEncoder;
use synapse_server::catalog::application::{
    CatalogService, ContentFetcher, ContentSourceDraft, ContentSourceRecord, ContentSources, FetchError,
    Fetched, Placements, RegistryError, SyncOutcome,
};
use synapse_server::catalog::domain::content_tree::PRIMARY_SOURCE_ID;
use synapse_server::catalog::infrastructure::{
    ContentCache, ContentSync, FileSystemContentRepository, MountOrder, MountedSources, SourceRoot,
};

// ── fakes ────────────────────────────────────────────────────────────────────

/// An in-memory registry: the SQL is covered by the gated Postgres IT.
#[derive(Default)]
struct FakeRegistry {
    rows: Mutex<Vec<ContentSourceRecord>>,
}

impl FakeRegistry {
    fn with(record: ContentSourceRecord) -> Self {
        Self {
            rows: Mutex::new(vec![record]),
        }
    }
}

impl ContentSources for FakeRegistry {
    async fn list(&self) -> Result<Vec<ContentSourceRecord>, RegistryError> {
        Ok(self.rows.lock().unwrap().clone())
    }
    async fn upsert(&self, _: &ContentSourceDraft) -> Result<ContentSourceRecord, RegistryError> {
        unimplemented!("not exercised here")
    }
    async fn remove(&self, _: &str) -> Result<bool, RegistryError> {
        unimplemented!("not exercised here")
    }
    async fn record_sync(&self, id: &str, outcome: &SyncOutcome) -> Result<(), RegistryError> {
        let mut rows = self.rows.lock().unwrap();
        if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
            match outcome {
                SyncOutcome::Landed(sha) => {
                    row.last_sha = Some(sha.clone());
                    row.last_error = None;
                }
                SyncOutcome::Failed(detail) => row.last_error = Some(detail.clone()),
            }
        }
        Ok(())
    }
}

/// Serves one archive at one commit, and counts how often the archive was actually pulled.
struct FakeFetcher {
    sha: String,
    archive: Vec<u8>,
    pulls: Mutex<usize>,
}

impl FakeFetcher {
    fn new(sha: &str, archive: Vec<u8>) -> Self {
        Self {
            sha: sha.to_owned(),
            archive,
            pulls: Mutex::new(0),
        }
    }
}

impl ContentFetcher for FakeFetcher {
    async fn fetch(&self, _: &str, _: &str, known_sha: Option<&str>) -> Result<Fetched, FetchError> {
        if known_sha == Some(self.sha.as_str()) {
            return Ok(Fetched::Unchanged);
        }
        *self.pulls.lock().unwrap() += 1;
        Ok(Fetched::Archive {
            sha: self.sha.clone(),
            bytes: self.archive.clone(),
        })
    }
}

/// A satellite guide repo, as GitHub would ship it: everything under one wrapping directory.
fn guide_archive(slug: &str, body: &str) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut add = |path: &str, content: &str| {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, path, content.as_bytes())
            .unwrap();
    };
    add(
        &format!("ani2fun-{slug}-guide-abc1234/book.json"),
        &format!(r#"{{"title":"Java","slug":"{slug}"}}"#),
    );
    add(
        &format!("ani2fun-{slug}-guide-abc1234/01-first-steps/01-what-java-is.md"),
        body,
    );
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&builder.into_inner().unwrap()).unwrap();
    encoder.finish().unwrap()
}

fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// A monorepo holding `programming-languages/` with a `java` book inside it.
fn seed_primary(root: &Path) {
    write(
        &root.join("programming-languages/category.json"),
        r#"{"title": "Programming Languages", "order": 6, "icon": "L"}"#,
    );
    write(
        &root.join("programming-languages/03-java/book.json"),
        r#"{"title": "Java", "slug": "java", "order": 7}"#,
    );
    write(
        &root.join("programming-languages/03-java/01-first-steps/01-what-java-is.md"),
        "---\ntitle: What Java Is\nsummary: s\n---\nFROM THE MONOREPO",
    );
}

fn record(grouping: &[&str]) -> ContentSourceRecord {
    ContentSourceRecord {
        id: "java-guide".to_owned(),
        repo: "ani2fun/java-guide".to_owned(),
        branch: "main".to_owned(),
        grouping: grouping.iter().map(|s| (*s).to_owned()).collect(),
        order: Some(7),
        enabled: true,
        last_sha: None,
        last_synced_at: None,
        last_error: None,
    }
}

/// The URL that must survive the split, byte for byte.
const URL: [&str; 4] = ["programming-languages", "java", "first-steps", "what-java-is"];

fn lesson_path() -> Vec<String> {
    URL.iter().map(|s| (*s).to_owned()).collect()
}

// ── the cutover ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_satellite_takes_over_at_the_identical_url_once_the_monorepo_lets_go() {
    let primary = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    seed_primary(primary.path());

    let registry = Arc::new(FakeRegistry::with(record(&["programming-languages"])));
    let fetcher = Arc::new(FakeFetcher::new(
        "abc1234",
        guide_archive(
            "java",
            "---\ntitle: What Java Is\nsummary: s\n---\nFROM THE SATELLITE",
        ),
    ));
    let mounted = MountedSources::new(vec![SourceRoot::new(PRIMARY_SOURCE_ID, primary.path())]);
    let placements = Placements::default();
    let sync = ContentSync::new(
        Arc::clone(&registry),
        Arc::clone(&fetcher),
        ContentCache::new(cache.path()),
        mounted.clone(),
        placements.clone(),
        MountOrder::pinned(
            vec![SourceRoot::new(PRIMARY_SOURCE_ID, primary.path())],
            Vec::new(),
        ),
    );
    let catalog = CatalogService::with_placements(
        FileSystemContentRepository::mounted(mounted.clone(), true),
        placements.clone(),
    );

    // ── 1. Register and sync while BOTH copies exist. Nothing may change for readers.
    sync.tick().await;
    let lesson = catalog.lesson(&lesson_path()).await.unwrap();
    assert_eq!(
        lesson.raw, "FROM THE MONOREPO",
        "first source wins: the satellite must not shadow the monorepo mid-migration"
    );

    // The satellite is on disk and verifiable even though it is not the one serving.
    assert!(cache.path().join("java-guide/current/book.json").exists());
    assert_eq!(*fetcher.pulls.lock().unwrap(), 1);

    // ── 2. The monorepo lets go. The satellite takes over at the SAME url.
    std::fs::remove_dir_all(primary.path().join("programming-languages/03-java")).unwrap();
    sync.tick().await;

    let lesson = catalog.lesson(&lesson_path()).await.unwrap();
    assert_eq!(lesson.raw, "FROM THE SATELLITE");
    assert_eq!(lesson.book.title, "Java");
    assert_eq!(
        lesson.prev_path.as_deref(),
        None,
        "the satellite's book starts at this lesson"
    );

    // The category the monorepo still declares keeps its own metadata.
    let index = catalog.index().await.unwrap();
    let category = index
        .entries
        .iter()
        .find(|e| e.slug() == "programming-languages")
        .expect("the grouping survives, declared by the monorepo");
    assert_eq!(category.slug(), "programming-languages");

    // ── 3. An unchanged head must not re-pull the archive.
    sync.tick().await;
    assert_eq!(
        *fetcher.pulls.lock().unwrap(),
        1,
        "the cheap head check is what keeps a 60s cadence affordable"
    );
}

/// A satellite that has never landed is an absent book, never a broken catalog.
#[tokio::test]
async fn a_source_that_cannot_be_fetched_leaves_the_rest_of_the_library_serving() {
    struct Broken;
    impl ContentFetcher for Broken {
        async fn fetch(&self, repo: &str, _: &str, _: Option<&str>) -> Result<Fetched, FetchError> {
            Err(FetchError::NotFound(repo.to_owned()))
        }
    }

    let primary = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    seed_primary(primary.path());

    let registry = Arc::new(FakeRegistry::with(record(&["programming-languages"])));
    let mounted = MountedSources::new(vec![SourceRoot::new(PRIMARY_SOURCE_ID, primary.path())]);
    let placements = Placements::default();
    let sync = ContentSync::new(
        Arc::clone(&registry),
        Arc::new(Broken),
        ContentCache::new(cache.path()),
        mounted.clone(),
        placements.clone(),
        MountOrder::pinned(
            vec![SourceRoot::new(PRIMARY_SOURCE_ID, primary.path())],
            Vec::new(),
        ),
    );
    let catalog =
        CatalogService::with_placements(FileSystemContentRepository::mounted(mounted, true), placements);

    sync.tick().await;

    // The monorepo's book is untouched...
    assert_eq!(
        catalog.lesson(&lesson_path()).await.unwrap().raw,
        "FROM THE MONOREPO"
    );
    // ...and the failure is on the row, where an admin will see it.
    let row = registry.list().await.unwrap().into_iter().next().unwrap();
    assert!(row.last_error.is_some(), "the failure must be recorded");
    assert_eq!(row.last_sha, None);
}

/// A rate limit is the one failure where retrying on the usual cadence makes things worse: each
/// tick spends a request that cannot succeed, off the quota that is trying to refill. The clock is
/// paused, so the five-minute window is stepped over rather than slept through.
#[tokio::test(start_paused = true)]
async fn a_rate_limited_source_is_left_alone_until_its_window_has_passed() {
    const WINDOW: u64 = 300;

    /// Rate-limits the FIRST fetch and serves the archive on every one after it — so the attempt
    /// count reads back exactly what the loop decided, with no second clock to keep in step.
    struct RateLimitedOnce {
        archive: Vec<u8>,
        attempts: Mutex<usize>,
    }

    impl ContentFetcher for RateLimitedOnce {
        async fn fetch(&self, _: &str, _: &str, _: Option<&str>) -> Result<Fetched, FetchError> {
            let attempt = {
                let mut attempts = self.attempts.lock().unwrap();
                *attempts += 1;
                *attempts
            };
            if attempt == 1 {
                return Err(FetchError::RateLimited { seconds: WINDOW });
            }
            Ok(Fetched::Archive {
                sha: "abc1234".to_owned(),
                bytes: self.archive.clone(),
            })
        }
    }

    let primary = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    seed_primary(primary.path());

    let registry = Arc::new(FakeRegistry::with(record(&["programming-languages"])));
    let fetcher = Arc::new(RateLimitedOnce {
        archive: guide_archive(
            "java",
            "---\ntitle: What Java Is\nsummary: s\n---\nFROM THE SATELLITE",
        ),
        attempts: Mutex::new(0),
    });
    let mounted = MountedSources::new(vec![SourceRoot::new(PRIMARY_SOURCE_ID, primary.path())]);
    let sync = ContentSync::new(
        Arc::clone(&registry),
        Arc::clone(&fetcher),
        ContentCache::new(cache.path()),
        mounted,
        Placements::default(),
        MountOrder::pinned(
            vec![SourceRoot::new(PRIMARY_SOURCE_ID, primary.path())],
            Vec::new(),
        ),
    );

    // ── 1. The forge says the quota is spent, with WINDOW seconds to go.
    sync.tick().await;
    assert_eq!(*fetcher.attempts.lock().unwrap(), 1);
    let row = registry.list().await.unwrap().into_iter().next().unwrap();
    assert_eq!(
        row.last_error.as_deref(),
        Some("rate limited, resets in 300s"),
        "backing off must not cost the admin the reason"
    );

    // ── 2. The next tick falls inside the window. The source must be skipped outright.
    sync.tick().await;
    assert_eq!(
        *fetcher.attempts.lock().unwrap(),
        1,
        "refetching inside the window is what keeps a throttled source throttled"
    );
    assert!(!cache.path().join("java-guide/current").exists());

    // ── 3. The window passes. One tick, one fetch, and the satellite lands.
    tokio::time::advance(Duration::from_secs(WINDOW + 1)).await;
    sync.tick().await;
    assert_eq!(
        *fetcher.attempts.lock().unwrap(),
        2,
        "the hold expires on its own — nothing else prompts the retry"
    );
    let row = registry.list().await.unwrap().into_iter().next().unwrap();
    assert_eq!(row.last_sha.as_deref(), Some("abc1234"));
    assert_eq!(row.last_error, None);
    assert!(cache.path().join("java-guide/current/book.json").exists());
}
