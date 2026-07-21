//! Tests for the pure catalog assembler — the convention rules, pinned.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::catalog::domain::catalog::{BookEntry, CatalogEntry};
use crate::catalog::domain::content_tree::{BookMeta, CategoryMeta, ContentEntry};

// ── fixture builders ──────────────────────────────────────────────────────────

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

fn cat_dir(name: &str, meta: CategoryMeta, children: Vec<ContentEntry>) -> ContentEntry {
    ContentEntry::Dir {
        name: name.to_owned(),
        book_meta: None,
        category_meta: Some(meta),
        children,
    }
}

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

// ── happy paths ───────────────────────────────────────────────────────────────

#[test]
fn empty_tree_walks_to_an_empty_catalog() {
    let result = walk(&[]).unwrap();
    assert!(result.catalog.entries.is_empty());
    assert!(result.lesson_files.is_empty());
}

#[test]
fn a_single_lesson_becomes_a_book_with_defaults() {
    let result = walk(&[book_dir(
        "dsa",
        BookMeta::default(),
        vec![file("01-intro.md", "hello")],
    )])
    .unwrap();
    let book = the_book(&result);
    assert_eq!(book.slug, "dsa");
    assert_eq!(book.title, "Dsa");
    assert_eq!(book.description, "");
    assert!(book.tags.is_empty());
    assert_eq!(book.estimated_reading_minutes, None);
    assert!(book.category_path.is_empty());
    assert_eq!(lesson_slugs(&book.entries), vec!["intro"]);
}

#[test]
fn editorial_sidecars_are_not_lessons() {
    let result = walk(&[book_dir(
        "b",
        BookMeta::default(),
        vec![
            file("01-two-sum.md", "x"),
            file("01-two-sum.editorial.md", "spoilers"),
        ],
    )])
    .unwrap();
    assert_eq!(lesson_slugs(&the_book(&result).entries), vec!["two-sum"]);
}

#[test]
fn lessons_sort_by_numeric_prefix_then_name_and_index_first() {
    let result = walk(&[book_dir(
        "b",
        BookMeta::default(),
        vec![
            file("10-later.md", "x"),
            file("02-second.md", "x"),
            file("index.md", "welcome"),
            file("unnumbered.md", "x"),
        ],
    )])
    .unwrap();
    assert_eq!(
        lesson_slugs(&the_book(&result).entries),
        vec!["index", "second", "later", "unnumbered"]
    );
}

#[test]
fn nested_chapters_build_a_tree_with_humanized_titles() {
    let result = walk(&[book_dir(
        "b",
        BookMeta::default(),
        vec![dir("01-linked-lists", vec![file("01-singly.md", "x")])],
    )])
    .unwrap();
    let BookEntry::Chapter {
        slug,
        title,
        order,
        entries,
    } = &the_book(&result).entries[0]
    else {
        panic!("expected a chapter");
    };
    assert_eq!(slug, "linked-lists");
    assert_eq!(title, "Linked Lists");
    assert_eq!(*order, Some(1));
    assert_eq!(lesson_slugs(entries), vec!["singly"]);
}

#[test]
fn book_meta_propagates_and_fallbacks_humanize() {
    let meta = BookMeta {
        title: Some("Grokking DSA".to_owned()),
        description: Some("desc".to_owned()),
        tags: Some(vec!["dsa".to_owned()]),
        estimated_reading_minutes: Some(90),
        order: Some(2),
        slug: None,
    };
    let result = walk(&[
        book_dir("02-grokking_dsa", meta, vec![file("a.md", "x")]),
        book_dir(
            "01-first-principles",
            BookMeta::default(),
            vec![file("a.md", "x")],
        ),
    ])
    .unwrap();
    let [CatalogEntry::Book(first), CatalogEntry::Book(second)] = &result.catalog.entries[..] else {
        panic!("expected two books");
    };
    // order 1 (from prefix) sorts before order 2 (from meta)
    assert_eq!(first.slug, "first-principles");
    assert_eq!(first.title, "First Principles");
    assert_eq!(first.order, Some(1));
    assert_eq!(second.title, "Grokking DSA");
    assert_eq!(second.order, Some(2));
}

#[test]
fn unordered_books_sort_last_by_name() {
    let result = walk(&[
        book_dir("zeta", BookMeta::default(), vec![file("a.md", "x")]),
        book_dir("alpha", BookMeta::default(), vec![file("a.md", "x")]),
        book_dir("01-numbered", BookMeta::default(), vec![file("a.md", "x")]),
    ])
    .unwrap();
    let slugs: Vec<&str> = result.catalog.entries.iter().map(CatalogEntry::slug).collect();
    assert_eq!(slugs, vec!["numbered", "alpha", "zeta"]);
}

#[test]
fn lesson_title_precedence_frontmatter_h1_humanized() {
    let result = walk(&[book_dir(
        "b",
        BookMeta::default(),
        vec![
            file("01-a.md", "---\ntitle: Fence Title\n---\n# H1"),
            file("02-b.md", "# H1 Title\nbody"),
            file("03-some-name.md", "plain"),
        ],
    )])
    .unwrap();
    let titles: Vec<&str> = the_book(&result)
        .entries
        .iter()
        .filter_map(|e| match e {
            BookEntry::Lesson(l) => Some(l.title.as_str()),
            BookEntry::Chapter { .. } => None,
        })
        .collect();
    assert_eq!(titles, vec!["Fence Title", "H1 Title", "Some Name"]);
}

#[test]
fn essential_defaults_true_and_frontmatter_overrides() {
    let result = walk(&[book_dir(
        "b",
        BookMeta::default(),
        vec![
            file("01-a.md", "plain"),
            file("02-b.md", "---\nessential: false\n---\nx"),
        ],
    )])
    .unwrap();
    let essentials: Vec<bool> = the_book(&result)
        .entries
        .iter()
        .filter_map(|e| match e {
            BookEntry::Lesson(l) => Some(l.essential),
            BookEntry::Chapter { .. } => None,
        })
        .collect();
    assert_eq!(essentials, vec![true, false]);
}

// ── categories ────────────────────────────────────────────────────────────────

#[test]
fn books_nest_under_categories_and_record_category_path() {
    let meta = CategoryMeta {
        title: Some("System Design".to_owned()),
        description: Some("d".to_owned()),
        order: Some(1),
        icon: Some("gear".to_owned()),
    };
    let result = walk(&[cat_dir(
        "01-system-design",
        meta,
        vec![book_dir("hld", BookMeta::default(), vec![file("a.md", "x")])],
    )])
    .unwrap();
    let CatalogEntry::Category(category) = &result.catalog.entries[0] else {
        panic!("expected a category");
    };
    assert_eq!(category.slug, "system-design");
    assert_eq!(category.title, "System Design");
    assert_eq!(category.icon.as_deref(), Some("gear"));
    assert_eq!(category.order, Some(1));
    assert_eq!(the_book(&result).category_path, vec!["system-design"]);
}

#[test]
fn category_fallbacks_humanize_and_take_prefix_order() {
    let result = walk(&[dir(
        "02-deep_dives",
        vec![book_dir("b", BookMeta::default(), vec![file("a.md", "x")])],
    )])
    .unwrap();
    let CatalogEntry::Category(category) = &result.catalog.entries[0] else {
        panic!("expected a category");
    };
    assert_eq!(category.title, "Deep Dives");
    assert_eq!(category.order, Some(2));
}

#[test]
fn sub_categories_nest_to_any_depth() {
    let result = walk(&[dir(
        "outer",
        vec![dir(
            "inner",
            vec![book_dir("b", BookMeta::default(), vec![file("a.md", "x")])],
        )],
    )])
    .unwrap();
    assert_eq!(the_book(&result).category_path, vec!["outer", "inner"]);
}

#[test]
fn empty_and_aux_only_categories_are_dropped() {
    let result = walk(&[
        dir("empty", vec![]),
        dir("aux-only", vec![dir("_media", vec![]), file("stray.md", "x")]),
        book_dir("b", BookMeta::default(), vec![file("a.md", "x")]),
    ])
    .unwrap();
    let slugs: Vec<&str> = result.catalog.entries.iter().map(CatalogEntry::slug).collect();
    assert_eq!(slugs, vec!["b"]);
}

#[test]
fn categories_and_books_interleave_by_order_then_name() {
    let result = walk(&[
        book_dir("03-book", BookMeta::default(), vec![file("a.md", "x")]),
        dir(
            "01-category",
            vec![book_dir("inner", BookMeta::default(), vec![file("a.md", "x")])],
        ),
        book_dir("02-mid", BookMeta::default(), vec![file("a.md", "x")]),
    ])
    .unwrap();
    let slugs: Vec<&str> = result.catalog.entries.iter().map(CatalogEntry::slug).collect();
    assert_eq!(slugs, vec!["category", "mid", "book"]);
}

// ── filtering ─────────────────────────────────────────────────────────────────

#[test]
fn duplicate_in_book_slug_paths_error_sorted() {
    let err = walk(&[book_dir(
        "b",
        BookMeta::default(),
        vec![file("01-two-sum.md", "x"), file("02-two-sum.md", "x")],
    )])
    .unwrap_err();
    assert_eq!(
        err,
        SynapseContentError::DuplicateLessonSlug {
            book_slug: "b".to_owned(),
            slugs: vec!["two-sum".to_owned()]
        }
    );
}

#[test]
fn duplicate_book_slugs_error_globally() {
    let err = walk(&[
        book_dir("01-dsa", BookMeta::default(), vec![file("a.md", "x")]),
        dir(
            "cat",
            vec![book_dir("02-dsa", BookMeta::default(), vec![file("a.md", "x")])],
        ),
    ])
    .unwrap_err();
    assert_eq!(err, SynapseContentError::DuplicateBookSlug("dsa".to_owned()));
}

#[test]
fn explicit_book_json_slug_overrides_folder_but_file_paths_keep_it() {
    let meta = BookMeta {
        slug: Some("Pretty Slug".to_owned()),
        ..BookMeta::default()
    };
    let result = walk(&[book_dir("01-ugly-folder", meta, vec![file("01-a.md", "x")])]).unwrap();
    assert_eq!(the_book(&result).slug, "pretty-slug");
    assert_eq!(
        result
            .lesson_files
            .get("pretty-slug")
            .and_then(|m| m.get("a"))
            .map(String::as_str),
        Some("01-ugly-folder/01-a.md")
    );
}

#[test]
fn chapter_depth_at_max_is_allowed_and_beyond_errors() {
    fn nest(depth: usize) -> ContentEntry {
        if depth == 0 {
            file("leaf.md", "x")
        } else {
            dir(&format!("s{depth}"), vec![nest(depth - 1)])
        }
    }
    assert!(walk(&[book_dir("ok", BookMeta::default(), vec![nest(MAX_CHAPTER_DEPTH)])]).is_ok());
    let err = walk(&[book_dir(
        "deep",
        BookMeta::default(),
        vec![nest(MAX_CHAPTER_DEPTH + 1)],
    )])
    .unwrap_err();
    assert!(matches!(err, SynapseContentError::MaxChapterDepthExceeded(path) if path.starts_with("s7/")));
}

#[test]
fn lesson_files_round_trip_real_folder_names() {
    let result = walk(&[dir(
        "01-learn",
        vec![book_dir(
            "02-dsa",
            BookMeta::default(),
            vec![dir("03-lists", vec![file("04-singly.md", "x")])],
        )],
    )])
    .unwrap();
    assert_eq!(
        result
            .lesson_files
            .get("dsa")
            .and_then(|m| m.get("lists/singly"))
            .map(String::as_str),
        Some("01-learn/02-dsa/03-lists/04-singly.md")
    );
}

// ── public helpers ────────────────────────────────────────────────────────────

#[test]
fn helper_edge_cases() {
    assert_eq!(slugify("Hello World!"), "hello-world");
    assert_eq!(slugify("foo--bar"), "foo-bar");
    assert_eq!(slugify("-trim-"), "trim");
    assert_eq!(slugify("keep_underscore"), "keep_underscore");
    assert_eq!(humanise("01-singly-linked-list.md"), "Singly Linked List");
    assert_eq!(humanise("10_deep.dive"), "Deep Dive");
    assert_eq!(strip_order_prefix("01-foo"), "foo");
    assert_eq!(strip_order_prefix("1.bar"), "bar");
    assert_eq!(strip_order_prefix("10_baz"), "baz");
    assert_eq!(strip_order_prefix("01foo"), "foo");
    assert_eq!(strip_order_prefix("nope"), "nope");
    assert!(lesson_path_like("a/b-c/d_e"));
    assert!(!lesson_path_like("a//b"));
    assert!(!lesson_path_like("a/../b"));
    assert!(!lesson_path_like(""));
    assert!(slug_like("ok-slug_1"));
    assert!(!slug_like("has space"));
    assert!(!slug_like(""));
}
