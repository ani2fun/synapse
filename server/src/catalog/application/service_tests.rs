//! The use cases over an instrumented in-memory repo.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::catalog::domain::content_tree::{BookMeta, ContentEntry};

// ── the instrumented stub ─────────────────────────────────────────────────────

#[derive(Default)]
struct StubRepo {
    version: Mutex<String>,
    tree: Vec<ContentEntry>,
    files: BTreeMap<String, String>,
    loads: AtomicUsize,
    reads: AtomicUsize,
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

    async fn load_tree(&self) -> Result<Vec<ContentEntry>, ContentError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(self.tree.clone())
    }

    async fn read_lesson(&self, path: &str) -> Result<String, ContentError> {
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
            "01-learn/02-dsa/02-lists/02-doubly.md".to_owned(),
            "doubly body".to_owned(),
        ),
        (
            "01-learn/02-dsa/02-lists/_c4-docs/reader.md".to_owned(),
            "---\ntitle: Reader\nkind: component\ntechnology: Laminar\n---\nHow it works.".to_owned(),
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
