//! Oracle: `LibrarySearchSpec` — the five ranking/flattening behaviors, natively tested.

#![allow(clippy::unwrap_used)]

use synapse_shared::catalog::{BookDto, BookEntryDto, CatalogEntryDto, CategoryDto, ChapterDto, LessonDto};

use super::*;

fn lesson(slug: &str, title: &str) -> BookEntryDto {
    BookEntryDto::Lesson(LessonDto {
        slug: slug.to_owned(),
        title: title.to_owned(),
        order: None,
        essential: false,
        lesson_kind: None,
    })
}

fn fixture() -> (SynapseIndexDto, Vec<BlogSummaryDto>) {
    let book = BookDto {
        slug: "dsa".to_owned(),
        title: "DSA".to_owned(),
        description: String::new(),
        tags: Vec::new(),
        estimated_reading_minutes: None,
        order: None,
        category_path: vec!["cat".to_owned()],
        entries: vec![BookEntryDto::Chapter(ChapterDto {
            slug: "arrays".to_owned(),
            title: "Arrays".to_owned(),
            order: None,
            entries: vec![
                lesson("two-sum", "Two Sum"),
                lesson("binary-search", "Binary Search"),
            ],
        })],
    };
    let index = SynapseIndexDto {
        entries: vec![CatalogEntryDto::Category(CategoryDto {
            slug: "cat".to_owned(),
            title: "Foundations".to_owned(),
            description: None,
            icon: None,
            order: None,
            entries: vec![CatalogEntryDto::Book(book)],
        })],
    };
    let blog = vec![BlogSummaryDto {
        slug: "hello".to_owned(),
        title: "Two Ferments".to_owned(),
        summary: None,
        published_at: "2026-06-01".to_owned(),
        tags: Vec::new(),
        read_minutes: None,
        eyebrow: None,
    }];
    (index, blog)
}

#[test]
fn flatten_yields_lessons_book_and_blog_with_breadcrumbs() {
    let (index, blog) = fixture();
    let all = entries(&index, &blog);
    let labels: Vec<&str> = all.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(labels, vec!["DSA", "Two Sum", "Binary Search", "Two Ferments"]);

    let two_sum = all.iter().find(|e| e.label == "Two Sum").unwrap();
    assert_eq!(two_sum.kind, Kind::Lesson);
    assert_eq!(two_sum.sublabel, "Foundations › DSA › Arrays");
    assert_eq!(
        two_sum.page,
        Page::Lesson(vec![
            "cat".into(),
            "dsa".into(),
            "arrays".into(),
            "two-sum".into()
        ])
    );
    let book = all.iter().find(|e| e.label == "DSA").unwrap();
    assert_eq!(book.kind, Kind::Book);
    assert_eq!(
        book.page,
        Page::Lesson(vec![
            "cat".into(),
            "dsa".into(),
            "arrays".into(),
            "two-sum".into()
        ]),
        "the book links to its first lesson"
    );
}

#[test]
fn a_word_start_match_beats_a_subsequence_match() {
    let (index, blog) = fixture();
    let all = entries(&index, &blog);
    let results = search("bi", &all);
    assert_eq!(results[0].label, "Binary Search");
}

#[test]
fn substring_matches_case_insensitively_across_lessons_and_blog() {
    let (index, blog) = fixture();
    let all = entries(&index, &blog);
    let labels: Vec<String> = search("two", &all).iter().map(|e| e.label.clone()).collect();
    assert!(labels.contains(&"Two Sum".to_owned()));
    assert!(labels.contains(&"Two Ferments".to_owned()));
}

#[test]
fn no_match_is_empty_and_empty_query_is_everything_capped() {
    let (index, blog) = fixture();
    let all = entries(&index, &blog);
    assert!(search("zzzzz", &all).is_empty());
    assert_eq!(search("", &all).len(), all.len());
}

#[test]
fn a_book_title_match_outranks_breadcrumb_only_lessons() {
    let (index, blog) = fixture();
    let all = entries(&index, &blog);
    let results = search("dsa", &all);
    assert_eq!(results[0].kind, Kind::Book, "the book itself surfaces first");
}
