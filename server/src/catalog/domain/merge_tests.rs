//! What only shows up when sources meet: grafting a satellite into a grouping, merging categories
//! declared in one repo with books that arrive from another, and resolving the collisions that a
//! content migration creates on purpose.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::catalog::domain::catalog::BookEntry;
use crate::catalog::domain::content_tree::{BookMeta, CategoryMeta, ContentEntry};
use crate::catalog::domain::resolver;

fn file(name: &str) -> ContentEntry {
    ContentEntry::File {
        name: name.to_owned(),
        content: "---\ntitle: T\n---\nbody".to_owned(),
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

fn category_dir(name: &str, meta: CategoryMeta, children: Vec<ContentEntry>) -> ContentEntry {
    ContentEntry::Dir {
        name: name.to_owned(),
        book_meta: None,
        category_meta: Some(meta),
        children,
    }
}

fn meta(slug: &str, order: Option<i32>) -> BookMeta {
    BookMeta {
        slug: Some(slug.to_owned()),
        order,
        ..BookMeta::default()
    }
}

/// A collection source: the primary checkout's shape.
fn collection(id: &str, children: Vec<ContentEntry>) -> SourceTree {
    SourceTree {
        id: id.to_owned(),
        book_meta: None,
        category_meta: None,
        children,
    }
}

/// A satellite guide repo: the root IS the book.
fn book_source(id: &str, meta: BookMeta, children: Vec<ContentEntry>) -> SourceTree {
    SourceTree {
        id: id.to_owned(),
        book_meta: Some(meta),
        category_meta: None,
        children,
    }
}

fn at(grouping: &[&str], source_id: &str, order: Option<i32>) -> Placement {
    Placement {
        source_id: source_id.to_owned(),
        grouping: grouping.iter().map(|s| (*s).to_owned()).collect(),
        order,
    }
}

fn book_slugs(walk: &WalkResult) -> Vec<&str> {
    resolver::all_books(&walk.catalog)
        .into_iter()
        .map(|b| b.slug.as_str())
        .collect()
}

fn book<'a>(walk: &'a WalkResult, slug: &str) -> &'a Book {
    resolver::all_books(&walk.catalog)
        .into_iter()
        .find(|b| b.slug == slug)
        .expect("book present")
}

// ─────────────────────────────────────────────────────────────────────────────
// MERGE OF ONE IS THE IDENTITY
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn one_collection_source_passes_through_untouched() {
    let tree = vec![
        dir(
            "01-learn",
            vec![book_dir("02-dsa", meta("dsa", None), vec![file("01-a.md")])],
        ),
        book_dir("03-solo", meta("solo", None), vec![file("01-b.md")]),
    ];
    let source = collection("main", tree.clone());

    let merged = assemble(&[source], &[]).unwrap();
    let direct = walker::walk(&tree).unwrap();

    assert_eq!(merged.catalog, direct.catalog);
    assert_eq!(merged.lesson_files, direct.lesson_files);
    assert!(merged.warnings.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// THE SATELLITE GRAFT
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn a_book_source_grafts_into_the_grouping_the_placement_names() {
    let spine = collection(
        "main",
        vec![category_dir(
            "06-programming-languages",
            CategoryMeta {
                title: Some("Programming Languages".to_owned()),
                icon: Some("💻".to_owned()),
                order: Some(6),
                ..CategoryMeta::default()
            },
            vec![book_dir(
                "02-python",
                meta("python", Some(6)),
                vec![file("01-a.md")],
            )],
        )],
    );
    let java = book_source(
        "java",
        meta("java", Some(7)),
        vec![dir("01-first-steps", vec![file("01-what-java-is.md")])],
    );

    let walk = assemble(&[spine, java], &[at(&["programming-languages"], "java", None)]).unwrap();

    // The URL is what matters: the satellite must land at the SAME path it had in the monorepo.
    let java_book = book(&walk, "java");
    assert_eq!(java_book.category_path, vec!["programming-languages".to_owned()]);
    assert_eq!(resolver::book_prefix(java_book), "programming-languages/java");
    assert_eq!(
        walk.lesson_files["java"]["first-steps/what-java-is"].path,
        "01-first-steps/01-what-java-is.md"
    );
    assert!(walk.warnings.is_empty());
}

#[test]
fn a_grafted_book_reads_from_its_own_source() {
    let spine = collection(
        "main",
        vec![book_dir("01-dsa", meta("dsa", None), vec![file("01-a.md")])],
    );
    let java = book_source("java", meta("java", None), vec![file("01-a.md")]);

    let walk = assemble(&[spine, java], &[]).unwrap();

    assert_eq!(walk.lesson_files["dsa"]["a"].source_id, "main");
    assert_eq!(walk.lesson_files["java"]["a"].source_id, "java");
}

#[test]
fn a_grouping_no_source_declares_is_synthesized_and_sorts_last() {
    let spine = collection(
        "main",
        vec![book_dir("01-dsa", meta("dsa", Some(1)), vec![file("01-a.md")])],
    );
    let sql = book_source("sql", meta("sql", None), vec![file("01-a.md")]);

    let walk = assemble(&[spine, sql], &[at(&["databases"], "sql", None)]).unwrap();

    let CatalogEntry::Category(category) = walk
        .catalog
        .entries
        .iter()
        .find(|e| e.slug() == "databases")
        .expect("synthesized category")
    else {
        panic!("expected a category");
    };
    assert_eq!(category.title, "Databases");
    assert_eq!(category.icon, None);
    assert_eq!(category.order, None);
    assert_eq!(book(&walk, "sql").category_path, vec!["databases".to_owned()]);
}

/// The end state of a migration: every book in a grouping has moved to its own repository, so the
/// spine holds the grouping's `category.json` and nothing else. That file is the only statement of
/// the title, icon and order anywhere — losing it silently demotes a real category to a synthesized
/// one, which sorts last and has no icon.
#[test]
fn a_declaration_outlives_the_books_that_left_the_source() {
    let spine = collection(
        "main",
        vec![
            book_dir("01-features", meta("features", Some(1)), vec![file("01-a.md")]),
            category_dir(
                "programming-languages",
                CategoryMeta {
                    title: Some("Programming Languages".to_owned()),
                    icon: Some("💻".to_owned()),
                    order: Some(6),
                    ..CategoryMeta::default()
                },
                vec![],
            ),
            book_dir("09-scratch", meta("scratch", Some(9)), vec![file("01-a.md")]),
        ],
    );
    let java = book_source("java", meta("java", Some(7)), vec![file("01-a.md")]);

    let walk = assemble(&[spine, java], &[at(&["programming-languages"], "java", None)]).unwrap();

    let CatalogEntry::Category(category) = walk
        .catalog
        .entries
        .iter()
        .find(|e| e.slug() == "programming-languages")
        .expect("the declared category survives having no books of its own")
    else {
        panic!("expected a category");
    };
    assert_eq!(
        category.icon.as_deref(),
        Some("💻"),
        "the declaration is not lost"
    );
    assert_eq!(category.order, Some(6));
    // Order 6 puts it between the two books — a synthesized category would have sorted last.
    let slugs: Vec<&str> = walk.catalog.entries.iter().map(CatalogEntry::slug).collect();
    assert_eq!(slugs, vec!["features", "programming-languages", "scratch"]);
}

/// Two sources nesting the same grouping are agreeing, not disputing — only two `category.json`
/// files are a redeclaration. (The positive case is pinned above; false positives get ignored.)
#[test]
fn nesting_the_same_grouping_is_not_a_redeclaration() {
    let spine = collection(
        "main",
        vec![category_dir(
            "programming-languages",
            CategoryMeta {
                title: Some("Programming Languages".to_owned()),
                icon: Some("💻".to_owned()),
                order: Some(6),
                ..CategoryMeta::default()
            },
            vec![book_dir(
                "02-python",
                meta("python", Some(6)),
                vec![file("01-a.md")],
            )],
        )],
    );
    // No category.json of its own — it just happens to nest the same grouping.
    let staged = collection(
        "local-only",
        vec![dir(
            "programming-languages",
            vec![book_dir("01-sql", meta("sql", Some(1)), vec![file("01-a.md")])],
        )],
    );

    let walk = assemble(&[spine, staged], &[] as &[Placement]).unwrap();

    assert!(walk.warnings.is_empty(), "{:?}", walk.warnings);
    assert_eq!(
        book(&walk, "sql").category_path,
        vec!["programming-languages".to_owned()]
    );
    let CatalogEntry::Category(category) = &walk.catalog.entries[0] else {
        panic!("expected a category");
    };
    assert_eq!(
        category.icon.as_deref(),
        Some("💻"),
        "the spine still furnishes it"
    );
}

/// …but a declaration nobody ever fills is still not a shelf.
#[test]
fn a_declared_category_no_source_fills_is_pruned() {
    let spine = collection(
        "main",
        vec![
            book_dir("01-features", meta("features", Some(1)), vec![file("01-a.md")]),
            category_dir("empty-shelf", CategoryMeta::default(), vec![]),
        ],
    );
    let walk = assemble(&[spine], &[] as &[Placement]).unwrap();
    let slugs: Vec<&str> = walk.catalog.entries.iter().map(CatalogEntry::slug).collect();
    assert_eq!(slugs, vec!["features"]);
}

#[test]
fn a_placement_order_overrides_the_books_own() {
    let java = book_source("java", meta("java", Some(7)), vec![file("01-a.md")]);
    let walk = assemble(&[java], &[at(&[], "java", Some(2))]).unwrap();
    assert_eq!(book(&walk, "java").order, Some(2));
}

#[test]
fn a_book_source_without_a_slug_falls_back_to_the_source_id_and_says_so() {
    let guide = book_source("java-guide", BookMeta::default(), vec![file("01-a.md")]);
    let walk = assemble(&[guide], &[]).unwrap();

    assert_eq!(book_slugs(&walk), vec!["java-guide"]);
    assert!(walk.warnings.contains(&CatalogWarning::BookSourceWithoutSlug {
        source_id: "java-guide".to_owned(),
    }));
}

// ─────────────────────────────────────────────────────────────────────────────
// COLLISIONS — the migration window
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn the_first_source_keeps_a_contested_book_slug_and_its_own_files() {
    // The migration window: `java` still in the monorepo AND registered as a satellite.
    let spine = collection(
        "main",
        vec![book_dir("03-java", meta("java", None), vec![file("01-a.md")])],
    );
    let satellite = book_source("java", meta("java", None), vec![file("09-different.md")]);

    let walk = assemble(&[spine, satellite], &[]).unwrap();

    assert_eq!(book_slugs(&walk), vec!["java"]);
    // The kept book must keep the KEPT source's paths — overwriting the file map is the subtle
    // form of this bug, where the catalog entry survives but the bodies come from the loser.
    assert_eq!(walk.lesson_files["java"]["a"].source_id, "main");
    assert_eq!(walk.lesson_files["java"]["a"].path, "03-java/01-a.md");
    assert!(!walk.lesson_files["java"].contains_key("different"));
    assert!(walk.warnings.contains(&CatalogWarning::DuplicateBookSlug {
        slug: "java".to_owned(),
        kept_source: "main".to_owned(),
        skipped_source: "java".to_owned(),
    }));
}

#[test]
fn a_grouping_left_empty_by_a_skipped_book_does_not_survive_as_a_bare_heading() {
    let spine = collection(
        "main",
        vec![book_dir("01-dsa", meta("dsa", None), vec![file("01-a.md")])],
    );
    let clash = book_source("dupe", meta("dsa", None), vec![file("01-a.md")]);

    let walk = assemble(&[spine, clash], &[at(&["orphaned"], "dupe", None)]).unwrap();

    assert!(walk.catalog.entries.iter().all(|e| e.slug() != "orphaned"));
}

#[test]
fn the_first_category_declaration_wins_whole_node() {
    let declare = |id: &str, title: &str, icon: &str| {
        collection(
            id,
            vec![category_dir(
                "01-languages",
                CategoryMeta {
                    title: Some(title.to_owned()),
                    icon: Some(icon.to_owned()),
                    order: Some(1),
                    ..CategoryMeta::default()
                },
                vec![book_dir(
                    &format!("01-{id}"),
                    meta(id, None),
                    vec![file("01-a.md")],
                )],
            )],
        )
    };

    let walk = assemble(
        &[
            declare("main", "Languages", "💻"),
            declare("other", "Langs", "🐍"),
        ],
        &[],
    )
    .unwrap();

    let CatalogEntry::Category(category) = &walk.catalog.entries[0] else {
        panic!("expected a category");
    };
    assert_eq!(category.title, "Languages");
    assert_eq!(category.icon.as_deref(), Some("💻"));
    assert!(walk.warnings.contains(&CatalogWarning::CategoryRedeclared {
        slug: "languages".to_owned(),
        kept_source: "main".to_owned(),
        ignored_source: "other".to_owned(),
    }));
}

#[test]
fn a_synthesized_category_is_upgraded_by_a_later_real_declaration() {
    // Order-free migration: the satellite lands before the spine declares its grouping.
    let satellite = book_source("java", meta("java", None), vec![file("01-a.md")]);
    let spine = collection(
        "main",
        vec![category_dir(
            "06-programming-languages",
            CategoryMeta {
                title: Some("Programming Languages".to_owned()),
                icon: Some("💻".to_owned()),
                order: Some(6),
                ..CategoryMeta::default()
            },
            vec![book_dir("02-python", meta("python", None), vec![file("01-a.md")])],
        )],
    );

    let walk = assemble(
        &[satellite, spine],
        &[at(&["programming-languages"], "java", None)],
    )
    .unwrap();

    let CatalogEntry::Category(category) = &walk.catalog.entries[0] else {
        panic!("expected a category");
    };
    assert_eq!(category.title, "Programming Languages");
    assert_eq!(category.icon.as_deref(), Some("💻"));
    assert_eq!(category.order, Some(6));
    assert_eq!(category.entries.len(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// ORDERING
// ─────────────────────────────────────────────────────────────────────────────

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

    let walk = assemble(&[spine, java], &[at(&["languages"], "java", None)]).unwrap();

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
