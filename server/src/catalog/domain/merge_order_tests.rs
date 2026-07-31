//! One question, from every direction: WHO decides where a book sits.
//!
//! A registered satellite is positioned by its row and by nothing else; the walked spine is
//! positioned by its own `book.json` and by nothing else; and a level that receives a graft
//! re-sorts around it. Its own file because `merge_tests` sits against the 500-line cap and
//! because this is a single question, separate from the merge's cross-source behaviour.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::fixtures::*;
use super::*;
use crate::catalog::domain::catalog::BookEntry;
use crate::catalog::domain::content_tree::{BookMeta, CategoryMeta, ContentEntry};

/// A satellite guide repo whose `book.json` still carries the `order` a migration left behind.
fn satellite(id: &str, slug: &str, book_json_order: Option<i32>) -> SourceTree {
    book_source(id, meta(slug, book_json_order), vec![file("01-intro.md")])
}

/// A registration row: the placement, at the top level.
fn row(source_id: &str, order: Option<i32>) -> Placement {
    at(&[], source_id, order)
}

fn order_of(walk: &WalkResult, slug: &str) -> Option<i32> {
    book(walk, slug).order
}

#[test]
fn a_rows_order_beats_the_book_json_it_disagrees_with() {
    let sources = [satellite("java-guide", "java", Some(99))];
    let walk = assemble(&sources, &[row("java-guide", Some(7))]).unwrap();
    assert_eq!(order_of(&walk, "java"), Some(7), "the row positions a satellite");
}

/// The case the fallback used to hide. A registration that says nothing about position means
/// unpositioned — NOT "keep whatever number the repository last carried". Without this, deleting
/// a satellite's stale `order` would silently move the book, which is the opposite of the
/// guarantee that makes the field safe to remove.
#[test]
fn a_row_without_an_order_does_not_inherit_the_book_jsons() {
    let sources = [satellite("java-guide", "java", Some(99))];
    let walk = assemble(&sources, &[row("java-guide", None)]).unwrap();
    assert_eq!(
        order_of(&walk, "java"),
        None,
        "an unpositioned row means unpositioned"
    );
}

/// Both halves of the same rule: dropping the field from a satellite's `book.json` changes
/// nothing, because the row was already the only thing being read.
#[test]
fn deleting_a_satellites_stale_order_is_a_no_op() {
    let with_it = assemble(
        &[satellite("java-guide", "java", Some(99))],
        &[row("java-guide", Some(7))],
    )
    .unwrap();
    let without_it = assemble(
        &[satellite("java-guide", "java", None)],
        &[row("java-guide", Some(7))],
    )
    .unwrap();
    assert_eq!(order_of(&with_it, "java"), order_of(&without_it, "java"));
    assert_eq!(order_of(&without_it, "java"), Some(7));
}

/// The other authority, unchanged: the spine has no registration row, so its own metadata is what
/// positions it. Its nesting IS the library.
#[test]
fn the_walked_spine_keeps_its_own_book_json_order() {
    let sources = [SourceTree {
        id: "synapse-content".to_owned(),
        book_meta: None,
        category_meta: None,
        children: vec![ContentEntry::Dir {
            name: "03-a-book".to_owned(),
            book_meta: Some(BookMeta {
                slug: Some("a-book".to_owned()),
                order: Some(3),
                ..BookMeta::default()
            }),
            category_meta: None,
            children: vec![file("01-intro.md")],
        }],
    }];
    let walk = assemble(&sources, &[]).unwrap();
    assert_eq!(order_of(&walk, "a-book"), Some(3));
}

/// A placement positions the SOURCE, not the shelves inside it — a book the satellite nests under
/// its own category keeps its own order, or every lesson shelf would collapse onto one number.
#[test]
fn a_book_nested_in_the_satellites_own_category_keeps_its_metadata_order() {
    let sources = [SourceTree {
        id: "dsa-guide".to_owned(),
        book_meta: None,
        category_meta: None,
        children: vec![ContentEntry::Dir {
            name: "01-arrays".to_owned(),
            book_meta: Some(BookMeta {
                slug: Some("arrays".to_owned()),
                order: Some(11),
                ..BookMeta::default()
            }),
            category_meta: None,
            children: vec![file("01-intro.md")],
        }],
    }];
    let walk = assemble(&sources, &[row("dsa-guide", Some(5))]).unwrap();
    assert_eq!(
        order_of(&walk, "arrays"),
        Some(5),
        "a source's own top-level book IS what the row positions"
    );
}

#[test]
fn a_grafted_level_re_sorts_by_order_then_slug() {
    let spine = collection(
        "main",
        vec![category_dir(
            "06-languages",
            CategoryMeta {
                order: Some(6),
                ..CategoryMeta::default()
            },
            vec![
                book_dir("02-python", meta("python", Some(6)), vec![file("01-a.md")]),
                book_dir("09-zig", meta("zig", Some(9)), vec![file("01-a.md")]),
            ],
        )],
    );
    let java = book_source("java", meta("java", Some(7)), vec![file("01-a.md")]);

    // The row carries the 7, not the `book.json` — a registered satellite is positioned by its
    // registration, so a fixture that left the row empty would be testing the old fallback rather
    // than the re-sort this is about.
    let walk = assemble(&[spine, java], &[at(&["languages"], "java", Some(7))]).unwrap();

    let CatalogEntry::Category(category) = &walk.catalog.entries[0] else {
        panic!("expected a category");
    };
    let slugs: Vec<&str> = category.entries.iter().map(CatalogEntry::slug).collect();
    assert_eq!(slugs, vec!["python", "java", "zig"]);
}

#[test]
fn the_lessons_inside_a_grafted_book_keep_their_numeric_order() {
    let java = book_source(
        "java",
        meta("java", None),
        vec![
            file("02-second.md"),
            file("01-first.md"),
            file("unnumbered.md"),
            file("index.md"),
        ],
    );

    let walk = assemble(&[java], &[]).unwrap();

    let slugs: Vec<&str> = book(&walk, "java")
        .entries
        .iter()
        .map(|e| match e {
            BookEntry::Lesson(lesson) => lesson.slug.as_str(),
            BookEntry::Chapter { slug, .. } => slug.as_str(),
        })
        .collect();
    // index first, then the numeric prefixes in order, and the unnumbered one last.
    assert_eq!(slugs, vec!["index", "first", "second", "unnumbered"]);
}
