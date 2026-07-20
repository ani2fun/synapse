//! Oracle: `CatalogResolverSpec` — a hand-built tree, no filesystem.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::catalog::domain::catalog::{Book, BookEntry, CatalogEntry, Category, Lesson};

fn lesson(slug: &str) -> Lesson {
    Lesson {
        slug: slug.to_owned(),
        title: slug.to_owned(),
        order: None,
        essential: true,
        description: None,
        kind: None,
    }
}

fn fixture() -> SynapseContentCatalog {
    let book = Book {
        slug: "dsa".to_owned(),
        title: "DSA".to_owned(),
        description: String::new(),
        tags: vec![],
        estimated_reading_minutes: None,
        order: None,
        category_path: vec!["learn".to_owned()],
        entries: vec![
            BookEntry::Lesson(lesson("intro")),
            BookEntry::Chapter {
                slug: "lists".to_owned(),
                title: "Lists".to_owned(),
                order: None,
                entries: vec![
                    BookEntry::Lesson(lesson("singly")),
                    BookEntry::Lesson(lesson("doubly")),
                ],
            },
        ],
    };
    SynapseContentCatalog {
        entries: vec![CatalogEntry::Category(Category {
            slug: "learn".to_owned(),
            title: "Learn".to_owned(),
            description: None,
            icon: None,
            order: None,
            entries: vec![CatalogEntry::Book(book)],
        })],
    }
}

fn path(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn descends_categories_then_chapters_to_a_lesson() {
    let catalog = fixture();
    let (book, in_book, lesson) =
        resolve_lesson(&catalog, &path(&["learn", "dsa", "lists", "singly"])).unwrap();
    assert_eq!(book.slug, "dsa");
    assert_eq!(in_book, "lists/singly");
    assert_eq!(lesson.slug, "singly");
}

#[test]
fn resolves_a_book_root_lesson() {
    let catalog = fixture();
    let (_, in_book, lesson) = resolve_lesson(&catalog, &path(&["learn", "dsa", "intro"])).unwrap();
    assert_eq!(in_book, "intro");
    assert_eq!(lesson.slug, "intro");
}

#[test]
fn chapter_book_prefix_missing_and_empty_paths_resolve_to_none() {
    let catalog = fixture();
    assert!(resolve_lesson(&catalog, &path(&["learn", "dsa", "lists"])).is_none());
    assert!(resolve_lesson(&catalog, &path(&["learn", "dsa"])).is_none());
    assert!(resolve_lesson(&catalog, &path(&["learn", "nope", "intro"])).is_none());
    assert!(resolve_lesson(&catalog, &path(&[])).is_none());
}

#[test]
fn reading_order_is_preorder_with_in_book_paths() {
    let catalog = fixture();
    let Some((book, _, _)) = resolve_lesson(&catalog, &path(&["learn", "dsa", "intro"])) else {
        panic!("fixture must resolve");
    };
    let order: Vec<String> = lessons_in_reading_order(book)
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    assert_eq!(order, vec!["intro", "lists/singly", "lists/doubly"]);
}
