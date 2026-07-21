//! What the walker REFUSES to index. One theme: a directory or file present on disk that must
//! never become a catalog entry — hidden and non-slug names, a book's aux dirs, and material
//! that is deliberately unpublishable (ADR-RS002).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::catalog::domain::catalog::{Book, BookEntry, CatalogEntry};
use crate::catalog::domain::content_tree::{BookMeta, ContentEntry};

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

fn book_dir(name: &str, meta: BookMeta, children: Vec<ContentEntry>) -> ContentEntry {
    ContentEntry::Dir {
        name: name.to_owned(),
        book_meta: Some(meta),
        category_meta: None,
        children,
    }
}

/// Duplicated from `walker_tests.rs` rather than shared: these are three-line fixture helpers,
/// and a `mod common` between two sibling test files would be more structure than they earn.
fn the_book(result: &WalkResult) -> &Book {
    fn find(entries: &[CatalogEntry]) -> Option<&Book> {
        entries.iter().find_map(|e| match e {
            CatalogEntry::Book(b) => Some(b),
            CatalogEntry::Category(c) => find(&c.entries),
        })
    }
    find(&result.catalog.entries).expect("fixture holds a book")
}

fn lesson_slugs(entries: &[BookEntry]) -> Vec<&str> {
    entries
        .iter()
        .filter_map(|e| match e {
            BookEntry::Lesson(l) => Some(l.slug.as_str()),
            BookEntry::Chapter { .. } => None,
        })
        .collect()
}

#[test]
fn hidden_nonslug_dirs_and_root_files_are_skipped() {
    let result = walk(&[
        dir(".git", vec![]),
        dir("_media", vec![]),
        dir(
            "not a slug!",
            vec![book_dir("x", BookMeta::default(), vec![file("a.md", "x")])],
        ),
        file("README.md", "top-level file"),
        book_dir("b", BookMeta::default(), vec![file("a.md", "x")]),
    ])
    .unwrap();
    let slugs: Vec<&str> = result.catalog.entries.iter().map(CatalogEntry::slug).collect();
    assert_eq!(slugs, vec!["b"]);
}

#[test]
fn reserved_aux_dirs_and_hidden_files_are_skipped_inside_books() {
    let result = walk(&[book_dir(
        "b",
        BookMeta::default(),
        vec![
            dir("examples", vec![file("snippet.md", "x")]),
            dir("01-examples", vec![file("snippet.md", "x")]),
            dir("c4", vec![file("model.md", "x")]),
            dir("_c4-docs", vec![file("reader.md", "x")]),
            dir(".hidden", vec![file("h.md", "x")]),
            file("_draft.md", "x"),
            file(".notes.md", "x"),
            file("data.tests.json", "{}"),
            file("01-real.md", "x"),
        ],
    )])
    .unwrap();
    let book = the_book(&result);
    assert_eq!(book.entries.len(), 1);
    assert_eq!(lesson_slugs(&book.entries), vec!["real"]);
}

// ── errors & edge rules ───────────────────────────────────────────────────────

// ── local-only is never content ────────────────────────────────────────────────

#[test]
fn a_local_only_dir_yields_no_books_however_well_formed() {
    // The content tree really does carry one of these, holding two complete books — lessons
    // adapted from a commercial course plus a SQL book — kept for personal study (ADR-RS002).
    // Without this rule the walker would index them: they would reach /api/synapse/index,
    // /sitemap.xml and lesson_view. Keeping them unpublished would then depend on the bytes
    // never being pushed, a property of another repository's .gitignore rather than a decision
    // this server enforces itself.
    let result = walk(&[
        dir(
            "local-only",
            vec![book_dir(
                "system-design-swiftly",
                BookMeta {
                    title: Some("System Design Swiftly".to_owned()),
                    ..Default::default()
                },
                vec![file("01-intro.md", "---\ntitle: Intro\n---\nbody")],
            )],
        ),
        book_dir(
            "dsa",
            BookMeta {
                title: Some("DSA".to_owned()),
                ..Default::default()
            },
            vec![file("01-intro.md", "---\ntitle: Intro\n---\nbody")],
        ),
    ])
    .unwrap();

    let slugs: Vec<&str> = result.catalog.entries.iter().map(CatalogEntry::slug).collect();
    assert_eq!(slugs, vec!["dsa"], "local-only must not appear at all");
    assert!(
        !result.lesson_files.contains_key("system-design-swiftly"),
        "and none of its lessons may be reachable by path either"
    );
}

#[test]
fn the_order_prefix_does_not_smuggle_local_only_back_in() {
    // RESERVED_AUX_DIRS is checked order-prefix-stripped, so `01-local-only` is excluded too —
    // adding an ordering prefix must not quietly re-publish 66 lessons.
    let result = walk(&[dir(
        "01-local-only",
        vec![book_dir(
            "swiftly",
            BookMeta {
                title: Some("Swiftly".to_owned()),
                ..Default::default()
            },
            vec![file("01-intro.md", "---\ntitle: Intro\n---\nbody")],
        )],
    )])
    .unwrap();
    assert!(
        result.catalog.entries.is_empty(),
        "an order prefix must not defeat the exclusion"
    );
}
