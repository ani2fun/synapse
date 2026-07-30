//! One rule, from both directions: a registered satellite is positioned by its row and by nothing
//! else, and the walked spine is positioned by its own `book.json` and by nothing else.
//!
//! Its own file because `merge_tests.rs` sits exactly on the 500-line cap, and because these are
//! about a single question — who decides — rather than about the merge's cross-source behaviour.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::catalog::domain::content_tree::{BookMeta, ContentEntry};
use crate::catalog::domain::resolver;

fn file(name: &str) -> ContentEntry {
    ContentEntry::File {
        name: name.to_owned(),
        content: "---\ntitle: T\n---\nbody".to_owned(),
    }
}

/// A satellite guide repo: the root IS the book, and `order` here is the field a migration leaves
/// behind.
fn satellite(id: &str, slug: &str, book_json_order: Option<i32>) -> SourceTree {
    SourceTree {
        id: id.to_owned(),
        book_meta: Some(BookMeta {
            slug: Some(slug.to_owned()),
            order: book_json_order,
            ..BookMeta::default()
        }),
        category_meta: None,
        children: vec![file("01-intro.md")],
    }
}

fn row(source_id: &str, order: Option<i32>) -> Placement {
    Placement {
        source_id: source_id.to_owned(),
        grouping: Vec::new(),
        order,
    }
}

fn order_of(walk: &WalkResult, slug: &str) -> Option<i32> {
    resolver::all_books(&walk.catalog)
        .into_iter()
        .find(|b| b.slug == slug)
        .expect("book present")
        .order
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
