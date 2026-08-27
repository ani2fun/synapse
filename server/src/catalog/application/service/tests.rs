//! The use cases over an instrumented in-memory repo.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;

use super::*;
use crate::catalog::domain::content_tree::{BookMeta, ContentEntry, PRIMARY_SOURCE_ID, SourceTree};

// ── the instrumented stub ─────────────────────────────────────────────────────

#[derive(Default)]
struct StubRepo {
    version: Mutex<String>,
    tree: Vec<ContentEntry>,
    files: BTreeMap<String, String>,
    loads: AtomicUsize,
    reads: AtomicUsize,
    /// When set, `load_sources` waits for a permit — which lets a test hold one rebuild open and
    /// watch what the readers arriving during it actually do. Without it the stub returns so fast
    /// that a "concurrent" test would pass whether or not anything is single-flighted.
    gate: Option<Arc<Semaphore>>,
}

impl StubRepo {
    fn bump_version(&self, v: &str) {
        v.clone_into(&mut self.version.lock().unwrap());
    }
}

impl ContentRepository for StubRepo {
    async fn content_version(&self) -> String {
        self.version.lock().unwrap().clone()
    }

    async fn load_sources(&self) -> Result<Vec<SourceTree>, ContentError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.acquire().await.expect("gate closed").forget();
        }
        Ok(vec![SourceTree {
            id: PRIMARY_SOURCE_ID.to_owned(),
            book_meta: None,
            category_meta: None,
            children: self.tree.clone(),
        }])
    }

    async fn read_lesson(&self, _source_id: &str, path: &str) -> Result<String, ContentError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| ContentError::NotFound(path.to_owned()))
    }
}

fn file(name: &str, content: &str) -> ContentEntry {
    ContentEntry::File {
        name: name.to_owned(),
        content: content.to_owned(),
    }
}

fn dir(name: &str, children: Vec<ContentEntry>) -> ContentEntry {
    ContentEntry::Dir {
        name: name.to_owned(),
        book_meta: None,
        category_meta: None,
        children,
    }
}

fn book_dir(name: &str, children: Vec<ContentEntry>) -> ContentEntry {
    ContentEntry::Dir {
        name: name.to_owned(),
        book_meta: Some(BookMeta::default()),
        category_meta: None,
        children,
    }
}

/// `learn/dsa` book: `01-intro.md`, then chapter `02-lists/{01-singly,02-doubly}.md` —
/// plus real file contents keyed by full paths, the way the FS adapter will key them.
fn fixture() -> StubRepo {
    let tree = vec![dir(
        "01-learn",
        vec![book_dir(
            "02-dsa",
            vec![
                file("01-intro.md", ""),
                dir(
                    "02-lists",
                    vec![file("01-singly.md", ""), file("02-doubly.md", "")],
                ),
            ],
        )],
    )];
    let files = BTreeMap::from([
        (
            "01-learn/02-dsa/01-intro.md".to_owned(),
            "# Intro\nwelcome".to_owned(),
        ),
        (
            "01-learn/02-dsa/02-lists/01-singly.md".to_owned(),
            "---\ntitle: Singly\nkind: problem\n---\nbody".to_owned(),
        ),
        (
            "01-learn/02-dsa/02-lists/01-singly.editorial.md".to_owned(),
            "the editorial".to_owned(),
        ),
        (
            // One sample case + one hidden judge case — the sidecar the judge reads in full.
            "01-learn/02-dsa/02-lists/01-singly.tests.json".to_owned(),
            r#"{"args":[{"id":"n","label":"n","type":"int"}],"cases":[
                {"args":{"n":"1"},"expected":"a","sample":true},
                {"args":{"n":"2"},"expected":"hidden"}
            ]}"#
            .to_owned(),
        ),
        (
            "01-learn/02-dsa/02-lists/02-doubly.md".to_owned(),
            "doubly body".to_owned(),
        ),
        (
            "01-learn/02-dsa/02-lists/_c4-docs/reader.md".to_owned(),
            "---\ntitle: Reader\nkind: component\ntechnology: Laminar\n---\nHow it works.".to_owned(),
        ),
        // A ```d2 boards walkthrough, drawn beside the lessons that show it.
        (
            "01-learn/02-dsa/02-lists/_d2/url-shortener/boards.json".to_owned(),
            r#"{"generator":1,"source":"76e32334","root":"root","boards":[],"warnings":[]}"#.to_owned(),
        ),
        (
            "01-learn/02-dsa/02-lists/_d2/url-shortener/container.svg".to_owned(),
            "<svg>container</svg>".to_owned(),
        ),
    ]);
    StubRepo {
        version: Mutex::new("v1".to_owned()),
        tree,
        files,
        ..StubRepo::default()
    }
}

fn path(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| (*s).to_owned()).collect()
}

// ── index & cache ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn index_walks_the_tree() {
    let service = CatalogService::new(fixture());
    let index = service.index().await.unwrap();
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].slug(), "learn");
}

#[tokio::test]
async fn index_rebuilds_only_when_the_version_moves() {
    let service = CatalogService::new(fixture());
    service.index().await.unwrap();
    service.index().await.unwrap();
    assert_eq!(service.repo.loads.load(Ordering::SeqCst), 1);
    service.repo.bump_version("v2");
    service.index().await.unwrap();
    assert_eq!(service.repo.loads.load(Ordering::SeqCst), 2);
}

/// The behaviour this exists for: readers arriving while the index is being rebuilt get the
/// PREVIOUS snapshot immediately, and only one rebuild runs.
///
/// Before, each of them ran a full rebuild of its own — correct, because the walk is idempotent,
/// and wasteful in proportion to how many readers arrived. Measured against the real catalog:
/// five concurrent readers, five rebuilds, on a pod with one CPU.
#[tokio::test]
async fn readers_during_a_rebuild_get_the_previous_snapshot_and_do_not_rebuild() {
    let gate = Arc::new(Semaphore::new(1)); // one permit: the warm-up build passes straight through
    let mut repo = fixture();
    repo.gate = Some(Arc::clone(&gate));
    let service = Arc::new(CatalogService::new(repo));

    service.index().await.unwrap();
    assert_eq!(service.repo.loads.load(Ordering::SeqCst), 1, "warm-up");

    // Invalidate. The gate is empty now, so whoever rebuilds next parks inside `load_sources`.
    service.repo.bump_version("v2");
    let building = tokio::spawn({
        let service = Arc::clone(&service);
        async move { service.index().await }
    });
    while service.repo.loads.load(Ordering::SeqCst) < 2 {
        tokio::task::yield_now().await;
    }

    // Four readers arrive mid-rebuild. Each must return the stale snapshot rather than queue
    // behind the rebuild or start one — so `loads` must not move.
    for _ in 0..4 {
        service
            .index()
            .await
            .expect("a reader mid-rebuild is served, not blocked");
    }
    assert_eq!(
        service.repo.loads.load(Ordering::SeqCst),
        2,
        "four readers during a rebuild must add no loads of their own"
    );

    gate.add_permits(1);
    building.await.unwrap().unwrap();
    assert_eq!(
        service.repo.loads.load(Ordering::SeqCst),
        2,
        "and the rebuild was the only one"
    );
}

/// The one case that must still wait: nothing has ever been built, so there is no stale snapshot
/// to hand back. A reader arriving then queues rather than being told the catalog is empty.
#[tokio::test]
async fn a_cold_start_waits_for_the_first_build_rather_than_serving_nothing() {
    let gate = Arc::new(Semaphore::new(0)); // the FIRST build parks
    let mut repo = fixture();
    repo.gate = Some(Arc::clone(&gate));
    let service = Arc::new(CatalogService::new(repo));

    let first = tokio::spawn({
        let service = Arc::clone(&service);
        async move { service.index().await }
    });
    while service.repo.loads.load(Ordering::SeqCst) < 1 {
        tokio::task::yield_now().await;
    }
    let second = tokio::spawn({
        let service = Arc::clone(&service);
        async move { service.index().await }
    });

    gate.add_permits(1);
    first.await.unwrap().unwrap();
    second
        .await
        .unwrap()
        .expect("the queued reader gets the first build, not an error");
    assert_eq!(
        service.repo.loads.load(Ordering::SeqCst),
        1,
        "the second reader waited for the first build instead of starting its own"
    );
}

// ── lessons ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn lesson_resolves_the_mirror_path_and_reads_the_file_path() {
    let service = CatalogService::new(fixture());
    let lesson = service.lesson(&path(&["learn", "dsa", "intro"])).await.unwrap();
    assert_eq!(lesson.lesson.slug, "intro");
    assert_eq!(lesson.frontmatter.title, "Intro");
    assert_eq!(lesson.raw, "# Intro\nwelcome");
    assert_eq!(lesson.prev_path, None);
    assert_eq!(lesson.next_path.as_deref(), Some("lists/singly"));
    assert_eq!(lesson.editorial, None);
}

#[tokio::test]
async fn prev_next_cross_chapter_boundaries_and_end_empty() {
    let service = CatalogService::new(fixture());
    let last = service
        .lesson(&path(&["learn", "dsa", "lists", "doubly"]))
        .await
        .unwrap();
    assert_eq!(last.prev_path.as_deref(), Some("lists/singly"));
    assert_eq!(last.next_path, None);
}

#[tokio::test]
async fn problem_lessons_join_their_editorial_sidecar() {
    let service = CatalogService::new(fixture());
    let lesson = service
        .lesson(&path(&["learn", "dsa", "lists", "singly"]))
        .await
        .unwrap();
    assert_eq!(lesson.frontmatter.kind.as_deref(), Some("problem"));
    assert_eq!(lesson.editorial.as_deref(), Some("the editorial"));
}

#[tokio::test]
async fn problem_lessons_serve_only_their_sample_tests() {
    let service = CatalogService::new(fixture());
    let lesson = service
        .lesson(&path(&["learn", "dsa", "lists", "singly"]))
        .await
        .unwrap();
    let tests = lesson.sample_tests.expect("a problem with a .tests.json sidecar");
    assert_eq!(tests.cases.len(), 1, "only the sample case is served");
    assert_eq!(tests.cases[0].args["n"], "1");
    assert!(!tests.cases[0].sample, "the served marker is cleared");
    // The hidden judge case (and its expected output) never reaches the payload.
    assert!(
        tests
            .cases
            .iter()
            .all(|c| c.expected.as_deref() != Some("hidden"))
    );
}

#[tokio::test]
async fn non_problem_lessons_have_no_sample_tests() {
    let service = CatalogService::new(fixture());
    let lesson = service.lesson(&path(&["learn", "dsa", "intro"])).await.unwrap();
    assert_eq!(lesson.sample_tests, None);
}

#[tokio::test]
async fn lesson_bodies_are_reread_every_call() {
    let service = CatalogService::new(fixture());
    service.lesson(&path(&["learn", "dsa", "intro"])).await.unwrap();
    service.lesson(&path(&["learn", "dsa", "intro"])).await.unwrap();
    assert_eq!(service.repo.reads.load(Ordering::SeqCst), 2);
    assert_eq!(service.repo.loads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn bad_paths_are_not_found() {
    let service = CatalogService::new(fixture());
    for bad in [
        path(&[]),
        path(&["learn", "../etc", "x"]),
        path(&["learn", "dsa", "lists"]),
        path(&["learn", "dsa", "missing"]),
    ] {
        assert!(
            matches!(service.lesson(&bad).await, Err(ContentError::NotFound(_))),
            "expected NotFound for {bad:?}"
        );
    }
}

#[tokio::test]
async fn convention_violations_surface_as_index_invalid() {
    let repo = StubRepo {
        version: Mutex::new("v1".to_owned()),
        tree: vec![
            book_dir("01-dsa", vec![file("a.md", "")]),
            book_dir("02-dsa", vec![file("a.md", "")]),
        ],
        ..StubRepo::default()
    };
    let service = CatalogService::new(repo);
    assert!(matches!(
        service.index().await,
        Err(ContentError::IndexInvalid(_))
    ));
}

// ── component docs ────────────────────────────────────────────────────────────

#[tokio::test]
async fn component_doc_reads_the_colocated_sidecar_by_leaf_id() {
    let service = CatalogService::new(fixture());
    let lesson_path = path(&["learn", "dsa", "lists", "singly"]);
    // The bare leaf and a container-view FQN resolve the same sidecar.
    for id in ["reader", "synapse.client.reader"] {
        let doc = service.component_doc(&lesson_path, id).await.unwrap();
        assert_eq!(doc.title.as_deref(), Some("Reader"), "id {id}");
        assert_eq!(doc.technology.as_deref(), Some("Laminar"));
        assert_eq!(doc.body, "How it works.");
    }
}

#[tokio::test]
async fn component_doc_rejects_bad_ids_unknown_lessons_and_absent_sidecars() {
    let service = CatalogService::new(fixture());
    let lesson_path = path(&["learn", "dsa", "lists", "singly"]);
    let reads_before = service.repo.reads.load(Ordering::SeqCst);
    assert!(matches!(
        service.component_doc(&lesson_path, "../../etc/passwd").await,
        Err(ContentError::NotFound(_))
    ));
    // Rejected before any read.
    assert_eq!(service.repo.reads.load(Ordering::SeqCst), reads_before);
    assert!(matches!(
        service
            .component_doc(&path(&["learn", "nope", "x"]), "reader")
            .await,
        Err(ContentError::NotFound(_))
    ));
    assert!(matches!(
        service.component_doc(&lesson_path, "unknown-component").await,
        Err(ContentError::NotFound(_))
    ));
}
