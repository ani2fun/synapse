//! Integration: the filesystem adapter + commit SHA against REAL temp dirs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use synapse_server::catalog::application::{CatalogService, ContentError, ContentRepository, Placements};
use synapse_server::catalog::domain::content_tree::PRIMARY_SOURCE_ID;
use synapse_server::catalog::domain::merge::Placement;
use synapse_server::catalog::infrastructure::{FileSystemContentRepository, SourceRoot, read_commit_sha};

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
            matches!(
                repo.read_lesson(PRIMARY_SOURCE_ID, bad).await,
                Err(ContentError::NotFound(_))
            ),
            "expected NotFound for {bad}"
        );
    }
    fs::remove_file(sibling).unwrap();

    let ok = repo
        .read_lesson(PRIMARY_SOURCE_ID, "01-learn/02-dsa/01-intro.md")
        .await
        .unwrap();
    assert_eq!(ok, "# Intro\nwelcome");
}

/// Reads NEVER fall through to another source. Two satellites with the same interior layout would
/// otherwise cross-serve each other's bodies and sidecars — the whole reason the id is carried.
#[tokio::test]
async fn read_lesson_refuses_an_unknown_source() {
    let tmp = tempfile::tempdir().unwrap();
    seed_content(tmp.path());
    let repo = FileSystemContentRepository::new(tmp.path(), true);

    assert!(matches!(
        repo.read_lesson("java", "01-learn/02-dsa/01-intro.md").await,
        Err(ContentError::NotFound(_))
    ));
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
    // The version NAMES its source: with several checkouts mounted, a bare SHA could not say
    // which one moved, and the index cache is keyed on this string.
    assert_eq!(repo.content_version().await, format!("{PRIMARY_SOURCE_ID}={SHA}"));
}

/// Any one source moving must invalidate the whole index — the cache holds one merged walk.
#[tokio::test]
async fn the_version_composes_every_mounted_source() {
    let main = tempfile::tempdir().unwrap();
    let java = tempfile::tempdir().unwrap();
    seed_content(main.path());
    write(&java.path().join("book.json"), r#"{"slug":"java"}"#);
    write(&java.path().join("01-first-steps/01-intro.md"), "hello");

    let repo = FileSystemContentRepository::over(
        vec![
            SourceRoot::new(PRIMARY_SOURCE_ID, main.path()),
            SourceRoot::new("java", java.path()),
        ],
        true,
    );

    let before = repo.content_version().await;
    assert!(before.contains("main="), "{before}");
    assert!(before.contains("java="), "{before}");

    write(&java.path().join("01-first-steps/02-more.md"), "added");
    assert_ne!(repo.content_version().await, before);
}

/// The same invariant on the PROD path, for the checkout shape that has no git metadata at all.
///
/// The test above passes with `auto_reload = true`, where the version is an mtime watermark that
/// notices any write. Prod reads commit SHAs instead, and a FETCHED satellite is an unpacked
/// tarball: `ContentCache` names each commit directory after its SHA and flips `current` onto it,
/// which is the only commit id on disk. A satellite that could not report one answered `static`
/// before and after every fetch, so the joined version never moved, the version-gated index cache
/// never rebuilt, and content that had genuinely landed stayed invisible until the process
/// restarted. This is that hole, closed.
#[tokio::test]
async fn prod_mode_moves_when_a_fetched_satellite_lands_a_new_commit() {
    const LANDED: &str = "d2dad749889b3dd4471356da8e47fba9fae42e21";

    let main = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    seed_content(main.path());
    write(&main.path().join(".git/HEAD"), "ref: refs/heads/main\n");
    write(&main.path().join(".git/refs/heads/main"), &format!("{SHA}\n"));

    // The cache layout, exactly as `ContentCache::publish` leaves it.
    let checkout = |sha: &str| {
        let commit_dir = cache.path().join("dsa-guide").join(sha);
        write(&commit_dir.join("book.json"), r#"{"slug":"dsa"}"#);
        let link = cache.path().join("dsa-guide/current");
        let _ = fs::remove_file(&link);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&commit_dir, &link).unwrap();
        link
    };

    let link = checkout("4ecf01b5a5acc83dd2b64ebd2c9f054de868aa53");
    let repo = FileSystemContentRepository::over(
        vec![
            SourceRoot::new(PRIMARY_SOURCE_ID, main.path()),
            // The mounted root is the STABLE `current` path — it is identical across fetches, so
            // the mount itself can never signal that anything moved.
            SourceRoot::new("dsa-guide", &link),
        ],
        false,
    );

    let before = repo.content_version().await;
    assert!(before.contains(&format!("{PRIMARY_SOURCE_ID}={SHA}")), "{before}");
    assert!(before.contains("dsa-guide="), "{before}");
    assert!(
        !before.contains("dsa-guide=static"),
        "a satellite must report the commit it is serving, not the fallback: {before}"
    );

    checkout(LANDED);

    let after = repo.content_version().await;
    assert!(after.contains(&format!("dsa-guide={LANDED}")), "{after}");
    assert_ne!(
        after, before,
        "a landed commit must move the version, or the index cache never rebuilds"
    );
}

/// The symptom itself, end to end: a satellite lands a lesson and a reader can open it, with no
/// restart. `CatalogService` memoises one merged walk behind the content version, so this passes
/// only if the version moved — it is the reason the version has to.
#[tokio::test]
async fn a_lesson_landed_by_a_satellite_is_readable_without_a_restart() {
    let main = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    // The spine carries no `dsa` book of its own: it is mounted first and wins the merge's
    // first-wins rule, so a competing slug here would shadow the satellite and the assertions
    // below would be measuring the wrong checkout.
    write(&main.path().join(".git/HEAD"), &format!("{SHA}\n"));

    let land = |sha: &str, lessons: &[(&str, &str)]| {
        let commit_dir = cache.path().join("dsa-guide").join(sha);
        write(&commit_dir.join("book.json"), r#"{"title":"DSA","slug":"dsa"}"#);
        for (path, body) in lessons {
            write(&commit_dir.join(path), body);
        }
        let link = cache.path().join("dsa-guide/current");
        let _ = fs::remove_file(&link);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&commit_dir, &link).unwrap();
        link
    };

    let link = land(
        "4ecf01b5a5acc83dd2b64ebd2c9f054de868aa53",
        &[("04-strings/05-isomorphic-string.md", "isomorphic body")],
    );
    let placements = Placements::default();
    placements.publish(vec![Placement {
        source_id: "dsa-guide".to_owned(),
        grouping: Vec::new(),
        order: Some(5),
    }]);
    let service = CatalogService::with_placements(
        FileSystemContentRepository::over(
            vec![
                SourceRoot::new(PRIMARY_SOURCE_ID, main.path()),
                SourceRoot::new("dsa-guide", &link),
            ],
            false,
        ),
        placements,
    );

    let path = |lesson: &str| {
        ["dsa", "strings", lesson]
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<String>>()
    };
    // The satellite is genuinely mounted and served before the flip — otherwise the assertion
    // below would pass for the wrong reason.
    assert_eq!(
        service.lesson(&path("isomorphic-string")).await.unwrap().raw,
        "isomorphic body"
    );
    let rotate = path("rotate-string");
    assert!(
        matches!(service.lesson(&rotate).await, Err(ContentError::NotFound(_))),
        "the lesson does not exist yet"
    );

    // The push lands: same `current` path, new commit behind it.
    land(
        "d2dad749889b3dd4471356da8e47fba9fae42e21",
        &[
            ("04-strings/05-isomorphic-string.md", "isomorphic body"),
            ("04-strings/06-rotate-string.md", "rotate body"),
        ],
    );

    let lesson = service
        .lesson(&rotate)
        .await
        .expect("the landed lesson must be readable without a restart");
    assert_eq!(lesson.raw, "rotate body");
}

/// The satellite shape: a root `book.json` makes the whole checkout one book, with its chapters
/// straight at the root and no category wrapper.
#[tokio::test]
async fn a_root_book_json_makes_the_checkout_one_book() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join("book.json"), r#"{"title":"Java","slug":"java"}"#);
    write(
        &tmp.path().join("01-first-steps/01-what-java-is.md"),
        "# What Java is",
    );

    // The book's opening lesson sits at the ROOT, which a collection root would ignore.
    write(&tmp.path().join("00-index.md"), "# Java");
    // Repo furniture shares that root and must not become a lesson.
    write(&tmp.path().join("README.md"), "how to contribute");

    let repo = FileSystemContentRepository::over(vec![SourceRoot::new("java", tmp.path())], true);
    let sources = repo.load_sources().await.unwrap();

    assert_eq!(sources.len(), 1);
    let root_meta = sources[0].book_meta.as_ref().expect("root book.json decoded");
    assert_eq!(root_meta.slug.as_deref(), Some("java"));

    let service = CatalogService::new(repo);
    let index = service.index().await.unwrap();
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].slug(), "java");

    let opening = service
        .lesson(&["java".to_owned(), "index".to_owned()])
        .await
        .expect("the root index.md is the book's first lesson");
    assert_eq!(opening.raw, "# Java");
    assert!(
        service
            .lesson(&["java".to_owned(), "readme".to_owned()])
            .await
            .is_err(),
        "README.md must not render as a lesson"
    );
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
