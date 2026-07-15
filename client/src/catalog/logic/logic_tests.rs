//! Native tests for the pure catalog logic.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use synapse_shared::catalog::{CategoryDto, ChapterDto};

fn lesson(slug: &str) -> BookEntryDto {
    BookEntryDto::Lesson(LessonDto {
        slug: slug.to_owned(),
        title: slug.to_owned(),
        order: None,
        essential: true,
    })
}

fn book() -> BookDto {
    BookDto {
        slug: "dsa".to_owned(),
        title: "DSA".to_owned(),
        description: String::new(),
        tags: vec![],
        estimated_reading_minutes: None,
        order: None,
        category_path: vec!["learn".to_owned()],
        entries: vec![
            lesson("intro"),
            BookEntryDto::Chapter(ChapterDto {
                slug: "lists".to_owned(),
                title: "Lists".to_owned(),
                order: None,
                entries: vec![lesson("singly")],
            }),
        ],
    }
}

fn index() -> SynapseIndexDto {
    SynapseIndexDto {
        entries: vec![CatalogEntryDto::Category(CategoryDto {
            slug: "learn".to_owned(),
            title: "Learn".to_owned(),
            description: None,
            icon: None,
            order: None,
            entries: vec![CatalogEntryDto::Book(book())],
        })],
    }
}

#[test]
fn reading_order_is_preorder_with_full_paths() {
    let paths: Vec<String> = reading_order(&book()).into_iter().map(|(p, _)| p).collect();
    assert_eq!(paths, vec!["learn/dsa/intro", "learn/dsa/lists/singly"]);
}

#[test]
fn first_lesson_path_is_the_cover_target() {
    assert_eq!(first_lesson_path(&book()).unwrap(), "learn/dsa/intro");
}

#[test]
fn book_of_descends_categories_to_the_owning_book() {
    let idx = index();
    let path: Vec<String> = ["learn", "dsa", "lists", "singly"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert_eq!(book_of(&idx, &path).unwrap().slug, "dsa");
    let missing: Vec<String> = ["learn", "nope", "x"].iter().map(|s| (*s).to_owned()).collect();
    assert!(book_of(&idx, &missing).is_none());
}
