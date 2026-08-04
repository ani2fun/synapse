//! What reaches the index from a real walk — and, just as importantly, what does not.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::catalog::domain::content_tree::{BookMeta, CategoryMeta};
use crate::catalog::domain::merge::{self, Placement};

fn file(name: &str, body: &str) -> ContentEntry {
    ContentEntry::File {
        name: name.to_owned(),
        content: body.to_owned(),
    }
}

fn book_source(id: &str, slug: &str, children: Vec<ContentEntry>) -> SourceTree {
    SourceTree {
        id: id.to_owned(),
        book_meta: Some(BookMeta {
            slug: Some(slug.to_owned()),
            title: Some(slug.to_uppercase()),
            ..BookMeta::default()
        }),
        category_meta: None,
        children,
    }
}

fn indexed(sources: &[SourceTree], placements: &[Placement]) -> SearchIndex {
    let walk = merge::assemble(sources, placements).expect("the fixture walks");
    index_of(sources, &walk)
}

#[test]
fn a_lesson_is_findable_by_its_prose_and_lands_at_its_real_url() {
    let sources = [book_source(
        "sql-guide",
        "sql",
        vec![ContentEntry::Dir {
            name: "05-window-functions".to_owned(),
            book_meta: None,
            category_meta: None,
            children: vec![file(
                "02-frames.md",
                "---\ntitle: Frames\n---\n\nA frame is bounded by ROWS BETWEEN.\n",
            )],
        }],
    )];
    let index = indexed(&sources, &[]);
    let hit = index
        .search("bounded", 5)
        .into_iter()
        .next()
        .expect("prose is searchable");
    assert_eq!(hit.title, "Frames");
    assert_eq!(hit.url, "sql/window-functions/frames");
    assert_eq!(hit.source_id, "sql-guide");
}

/// The reason this bridge exists at all: a satellite's own walk knows nothing about where it was
/// grafted, so the breadcrumb has to come from the MERGED catalog.
#[test]
fn a_grafted_satellite_carries_its_placement_in_the_breadcrumb() {
    let spine = SourceTree {
        id: "main".to_owned(),
        book_meta: None,
        category_meta: None,
        children: vec![ContentEntry::Dir {
            name: "06-programming-languages".to_owned(),
            book_meta: None,
            category_meta: Some(CategoryMeta {
                title: Some("Programming Languages".to_owned()),
                ..CategoryMeta::default()
            }),
            children: vec![],
        }],
    };
    let sql = book_source(
        "sql-guide",
        "sql",
        vec![file("01-intro.md", "Prose about joins.")],
    );
    let placement = Placement {
        source_id: "sql-guide".to_owned(),
        grouping: vec!["programming-languages".to_owned()],
        order: Some(8),
    };

    let index = indexed(&[spine, sql], &[placement]);
    let hit = index.search("joins", 5).into_iter().next().expect("a hit");
    assert_eq!(
        hit.breadcrumb,
        vec!["Programming Languages".to_owned(), "SQL".to_owned()],
        "titles, not slugs, and the grafted path — not the satellite's own idea of where it is"
    );
    assert_eq!(hit.url, "programming-languages/sql/intro");
}

/// Membership follows `lesson_files`, so the losing copy of a contested slug is absent from the
/// index for the same reason it is absent from the site — not by a second rule that could drift.
#[test]
fn the_book_that_lost_a_slug_collision_is_not_indexed() {
    let winner = book_source("main", "sql", vec![file("01-intro.md", "canonical prose here")]);
    let loser = book_source(
        "sql-guide",
        "sql",
        vec![file("01-intro.md", "shadowed prose here")],
    );
    let sources = [winner, loser];

    // Pin that the collision actually happened, or this would pass on a fixture that never
    // contested anything.
    let walk = merge::assemble(&sources, &[]).expect("the fixture walks");
    assert!(
        walk.warnings.iter().any(|w| matches!(
            w,
            crate::catalog::domain::catalog::CatalogWarning::DuplicateBookSlug { .. }
        )),
        "the fixture must contest a slug for this test to mean anything"
    );

    let index = index_of(&sources, &walk);
    assert_eq!(
        index.search("canonical", 5).len(),
        1,
        "the winner is served and indexed"
    );
    assert!(
        index.search("shadowed", 5).is_empty(),
        "the skipped copy is unreachable, so it must be unsearchable"
    );
}

/// Every source is indexed, not just the first — the whole point of searching a merged library.
#[test]
fn lessons_from_several_sources_are_all_indexed() {
    let spine = SourceTree {
        id: "main".to_owned(),
        book_meta: None,
        category_meta: None,
        children: vec![ContentEntry::Dir {
            name: "01-features".to_owned(),
            book_meta: Some(BookMeta {
                slug: Some("features".to_owned()),
                title: Some("Features".to_owned()),
                ..BookMeta::default()
            }),
            category_meta: None,
            children: vec![file("01-a.md", "spine prose mentions telemetry")],
        }],
    };
    let java = book_source(
        "java-guide",
        "java",
        vec![file("01-a.md", "java prose mentions telemetry")],
    );

    let index = indexed(&[spine, java], &[]);
    let sources: Vec<String> = index
        .search("telemetry", 10)
        .into_iter()
        .map(|hit| hit.source_id)
        .collect();
    assert_eq!(sources.len(), 2);
    assert!(sources.contains(&"main".to_owned()) && sources.contains(&"java-guide".to_owned()));
}

#[test]
fn a_frontmatter_summary_reaches_the_index() {
    let sources = [book_source(
        "guide",
        "guide",
        vec![file(
            "01-a.md",
            "---\ntitle: Latency\nsummary: Percentiles and tails\n---\n\nBody without the word.\n",
        )],
    )];
    let index = indexed(&sources, &[]);
    assert_eq!(index.search("percentiles", 5).len(), 1);
}

#[test]
fn an_empty_library_indexes_nothing() {
    let index = indexed(&[], &[]);
    assert!(index.is_empty());
    assert!(index.search("anything", 5).is_empty());
}
