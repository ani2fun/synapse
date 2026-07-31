//! The builders both merge suites drive: content trees, source shapes and placements, plus the
//! two readers that pull a book back out of a walk.
//!
//! Shared rather than duplicated because `merge_tests` and `merge_order_tests` ask different
//! questions of the SAME fixtures — a satellite grafted under a grouping is the setup for "who
//! wins a slug" and for "who decides the order" alike. Two copies would drift, and the second
//! copy had already started to (`satellite`/`row` were near-twins of `book_source`/`at`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(dead_code)] // each suite drives the subset it needs

use super::*;
use crate::catalog::domain::content_tree::{BookMeta, CategoryMeta, ContentEntry};
use crate::catalog::domain::resolver;

pub(super) fn file(name: &str) -> ContentEntry {
    ContentEntry::File {
        name: name.to_owned(),
        content: "---\ntitle: T\n---\nbody".to_owned(),
    }
}

pub(super) fn dir(name: &str, children: Vec<ContentEntry>) -> ContentEntry {
    ContentEntry::Dir {
        name: name.to_owned(),
        book_meta: None,
        category_meta: None,
        children,
    }
}

pub(super) fn book_dir(name: &str, meta: BookMeta, children: Vec<ContentEntry>) -> ContentEntry {
    ContentEntry::Dir {
        name: name.to_owned(),
        book_meta: Some(meta),
        category_meta: None,
        children,
    }
}

pub(super) fn category_dir(name: &str, meta: CategoryMeta, children: Vec<ContentEntry>) -> ContentEntry {
    ContentEntry::Dir {
        name: name.to_owned(),
        book_meta: None,
        category_meta: Some(meta),
        children,
    }
}

pub(super) fn meta(slug: &str, order: Option<i32>) -> BookMeta {
    BookMeta {
        slug: Some(slug.to_owned()),
        order,
        ..BookMeta::default()
    }
}

/// A collection source: the primary checkout's shape.
pub(super) fn collection(id: &str, children: Vec<ContentEntry>) -> SourceTree {
    SourceTree {
        id: id.to_owned(),
        book_meta: None,
        category_meta: None,
        children,
    }
}

/// A satellite guide repo: the root IS the book.
pub(super) fn book_source(id: &str, meta: BookMeta, children: Vec<ContentEntry>) -> SourceTree {
    SourceTree {
        id: id.to_owned(),
        book_meta: Some(meta),
        category_meta: None,
        children,
    }
}

pub(super) fn at(grouping: &[&str], source_id: &str, order: Option<i32>) -> Placement {
    Placement {
        source_id: source_id.to_owned(),
        grouping: grouping.iter().map(|s| (*s).to_owned()).collect(),
        order,
    }
}

pub(super) fn book_slugs(walk: &WalkResult) -> Vec<&str> {
    resolver::all_books(&walk.catalog)
        .into_iter()
        .map(|b| b.slug.as_str())
        .collect()
}

pub(super) fn book<'a>(walk: &'a WalkResult, slug: &str) -> &'a Book {
    resolver::all_books(&walk.catalog)
        .into_iter()
        .find(|b| b.slug == slug)
        .expect("book present")
}
