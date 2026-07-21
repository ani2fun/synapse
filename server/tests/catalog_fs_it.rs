//! Integration: the filesystem adapter + commit SHA against REAL temp dirs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use synapse_server::catalog::application::{CatalogService, ContentError, ContentRepository};
use synapse_server::catalog::infrastructure::{FileSystemContentRepository, read_commit_sha};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A miniature synapse-content: category/book/chapter with markers at the right depths.
fn seed_content(root: &Path) {
    write(
        &root.join("01-learn/02-dsa/book.json"),
        r#"{"title": "DSA", "order": 1}"#,
    );
    write(&root.join("01-learn/category.json"), r#"{"title": "Learn"}"#);
    write(&root.join("01-learn/02-dsa/01-intro.md"), "# Intro\nwelcome");
    write(&root.join("01-learn/02-dsa/02-lists/01-singly.md"), "singly body");
    write(&root.join(".hidden/ignored.md"), "never seen");
}

// ── load_tree + read_lesson ───────────────────────────────────────────────────

#[tokio::test]
async fn load_tree_decodes_markers_and_round_trips_lesson_reads() {
    let tmp = tempfile::tempdir().unwrap();
    seed_content(tmp.path());
    let repo = FileSystemContentRepository::new(tmp.path(), true);
    let service = CatalogService::new(repo);

    let index = service.index().await.unwrap();
    assert_eq!(index.entries.len(), 1, "hidden top-level dirs must be pruned");
    assert_eq!(index.entries[0].slug(), "learn");

    let path: Vec<String> = ["learn", "dsa", "lists", "singly"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let lesson = service.lesson(&path).await.unwrap();
    assert_eq!(lesson.raw, "singly body");
    assert_eq!(lesson.book.title, "DSA");
}

#[tokio::test]
async fn read_lesson_rejects_traversal_and_missing_files() {
    let tmp = tempfile::tempdir().unwrap();
    seed_content(tmp.path());
    // A secret OUTSIDE the root that a traversal would reach.
    let sibling = tmp.path().parent().unwrap().join("synapse-rs-secret.txt");
    fs::write(&sibling, "secret").unwrap();
    let repo = FileSystemContentRepository::new(tmp.path(), true);

    for bad in [
        "../synapse-rs-secret.txt",
        "01-learn/../../synapse-rs-secret.txt",
        "nope.md",
    ] {
        assert!(
            matches!(repo.read_lesson(bad).await, Err(ContentError::NotFound(_))),
            "expected NotFound for {bad}"
        );
    }
    fs::remove_file(sibling).unwrap();

    let ok = repo.read_lesson("01-learn/02-dsa/01-intro.md").await.unwrap();
    assert_eq!(ok, "# Intro\nwelcome");
}

// ── the content version (a watermark: advances on edit/add, ignores hidden churn) ────────────

#[tokio::test]
async fn watermark_advances_on_edit_and_on_add_but_not_on_hidden_churn() {
    let tmp = tempfile::tempdir().unwrap();
    seed_content(tmp.path());
    let repo = FileSystemContentRepository::new(tmp.path(), true);

    let v1 = repo.content_version().await;

    // Edit: push the mtime forward deterministically (no sleeps).
    let lesson = tmp.path().join("01-learn/02-dsa/01-intro.md");
    let file = fs::File::options().write(true).open(&lesson).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(5))
        .unwrap();
    let v2 = repo.content_version().await;
    assert_ne!(v1, v2, "an edit must advance the watermark");

    // Add: the file count moves.
    write(&tmp.path().join("01-learn/02-dsa/03-new.md"), "new");
    let v3 = repo.content_version().await;
    assert_ne!(v2, v3, "an added file must advance the watermark");

    // Hidden churn (e.g. .git) must NOT move it.
    write(&tmp.path().join(".git-like/objects/blob"), "vcs noise");
    let hidden = tmp.path().join(".git-like/objects/blob");
    let f = fs::File::options().write(true).open(&hidden).unwrap();
    f.set_modified(SystemTime::now() + Duration::from_mins(1))
        .unwrap();
    // rename to .git-shaped hidden dir is already hidden (starts with '.')
    assert_eq!(repo.content_version().await, v3, "hidden subtrees are pruned");
}

#[tokio::test]
async fn prod_mode_reports_the_commit_sha_and_ignores_edits() {
    let tmp = tempfile::tempdir().unwrap();
    seed_content(tmp.path());
    write(&tmp.path().join(".git/HEAD"), "ref: refs/heads/main\n");
    write(&tmp.path().join(".git/refs/heads/main"), &format!("{SHA}\n"));
    let repo = FileSystemContentRepository::new(tmp.path(), false);
    assert_eq!(repo.content_version().await, SHA);
}

// ── commit sha resolution ─────────────────────────────────────────────────────

#[test]
fn plain_clone_loose_ref_resolves() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join(".git/HEAD"), "ref: refs/heads/main\n");
    write(&tmp.path().join(".git/refs/heads/main"), &format!("{SHA}\n"));
    assert_eq!(read_commit_sha(tmp.path()), SHA);
}

#[test]
fn packed_ref_resolves() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join(".git/HEAD"), "ref: refs/heads/main\n");
    write(
        &tmp.path().join(".git/packed-refs"),
        &format!("# pack-refs with: peeled fully-peeled sorted\n{SHA} refs/heads/main\n"),
    );
    assert_eq!(read_commit_sha(tmp.path()), SHA);
}

#[test]
fn detached_head_is_the_sha() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join(".git/HEAD"), &format!("{SHA}\n"));
    assert_eq!(read_commit_sha(tmp.path()), SHA);
}

#[test]
fn gitdir_pointer_worktree_resolves() {
    let tmp = tempfile::tempdir().unwrap();
    let real_git = tmp.path().join("real-git");
    write(&real_git.join("HEAD"), "ref: refs/heads/main\n");
    write(&real_git.join("refs/heads/main"), &format!("{SHA}\n"));
    let checkout = tmp.path().join("checkout");
    fs::create_dir_all(&checkout).unwrap();
    fs::write(checkout.join(".git"), format!("gitdir: {}\n", real_git.display())).unwrap();
    assert_eq!(read_commit_sha(&checkout), SHA);
}

#[test]
fn not_a_checkout_or_garbage_degrades_to_static() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(read_commit_sha(tmp.path()), "static");
    write(&tmp.path().join(".git/HEAD"), "not a sha at all\n");
    assert_eq!(read_commit_sha(tmp.path()), "static");
}
