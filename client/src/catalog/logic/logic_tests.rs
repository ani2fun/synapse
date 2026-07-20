//! Native tests for the pure catalog logic.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use synapse_shared::catalog::{CategoryDto, ChapterDto};

fn lesson(slug: &str) -> BookEntryDto {
    entry(slug, None)
}

/// A `kind: problem` lesson — what `chapter_problems` counts.
fn problem(slug: &str) -> BookEntryDto {
    entry(slug, Some("problem"))
}

fn entry(slug: &str, kind: Option<&str>) -> BookEntryDto {
    BookEntryDto::Lesson(LessonDto {
        slug: slug.to_owned(),
        title: slug.to_owned(),
        order: None,
        essential: true,
        lesson_kind: kind.map(str::to_owned),
    })
}

fn chapter(slug: &str, entries: Vec<BookEntryDto>) -> BookEntryDto {
    BookEntryDto::Chapter(ChapterDto {
        slug: slug.to_owned(),
        title: slug.to_owned(),
        order: None,
        entries,
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

#[test]
fn find_book_descends_categories_by_globally_unique_slug() {
    let idx = index();
    assert_eq!(find_book(&idx, "dsa").unwrap().title, "DSA");
    assert!(find_book(&idx, "nope").is_none());
}

#[test]
fn card_counts_lessons_recursively_and_chapters_directly() {
    let b = book();
    assert_eq!(lesson_count(&b), 2);
    assert_eq!(chapter_count(&b), 1);
}

fn hop(tag: &str, classes: &str, id: Option<&str>) -> C4PathHop {
    (tag.to_owned(), classes.to_owned(), id.map(str::to_owned))
}

#[test]
fn a_node_body_click_resolves_to_its_dotted_fqn() {
    let path = vec![
        hop("DIV", "likec4-element", None),
        hop(
            "DIV",
            "react-flow__node react-flow__node-element",
            Some("btPersonal.btSmallWeb"),
        ),
        hop("DIV", "react-flow__pane", None),
    ];
    assert_eq!(resolve_c4_node(&path).as_deref(), Some("btPersonal.btSmallWeb"));
}

#[test]
fn a_button_before_the_node_is_likec4s_own_control() {
    let path = vec![
        hop("BUTTON", "mantine-ActionIcon-root", None),
        hop("DIV", "react-flow__node", Some("sfClient")),
    ];
    assert_eq!(resolve_c4_node(&path), None);
}

#[test]
fn edges_and_token_substrings_never_resolve() {
    let edge = vec![hop("G", "react-flow__edge", Some("hash-1a2b"))];
    assert_eq!(resolve_c4_node(&edge), None);
    let substring = vec![hop("DIV", "react-flow__node-toolbar", Some("x"))];
    assert_eq!(resolve_c4_node(&substring), None);
    let empty_id = vec![hop("DIV", "react-flow__node", Some(""))];
    assert_eq!(resolve_c4_node(&empty_id), None);
}

#[test]
fn prune_keeps_matching_lessons_and_whole_matching_chapters() {
    let entries = book().entries;
    let hits = prune_entries(&entries, "singly");
    assert_eq!(hits.len(), 1);
    match &hits[0] {
        BookEntryDto::Chapter(c) => assert_eq!(c.entries.len(), 1),
        BookEntryDto::Lesson(_) => panic!("expected the surviving chapter"),
    }
    // A matching CHAPTER title keeps all its lessons.
    let all = prune_entries(&entries, "lists");
    match &all[0] {
        BookEntryDto::Chapter(c) => assert_eq!(c.entries.len(), 1),
        BookEntryDto::Lesson(_) => panic!("expected the chapter"),
    }
    assert!(prune_entries(&entries, "zzz").is_empty());
    assert_eq!(prune_entries(&entries, "  ").len(), entries.len());
}

#[test]
fn spread_de_overlaps_and_clamps_fractions() {
    let out = spread_fractions(&[0.10, 0.11, 0.12]);
    assert!(out[1] - out[0] >= 0.05 - 1e-9);
    assert!(out[2] - out[1] >= 0.05 - 1e-9);
    let edges = spread_fractions(&[0.0, 1.0]);
    assert!(edges[0] >= 0.05 - 1e-9 && edges[1] <= 0.95 + 1e-9);
    assert!(spread_fractions(&[]).is_empty());
}

#[test]
fn problem_split_divides_at_the_first_top_level_details() {
    let raw = "The problem.\n\n```txt\n<details inside a fence>\n```\n\n<details>\n<summary>Editorial</summary>\nanswer\n</details>";
    let (desc, editorial) = problem_content_split(raw);
    assert!(desc.ends_with("```"));
    assert!(editorial.starts_with("<details>"));
    let (all, none) = problem_content_split("No editorial here.");
    assert_eq!(all, "No editorial here.");
    assert!(none.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// CHAPTER PROBLEMS — the problem-page counter's source of truth
// ─────────────────────────────────────────────────────────────────────────────

/// Two problem chapters plus prose, so sibling bleed and prose contamination both have a
/// chance to show up.
fn problem_book() -> BookDto {
    BookDto {
        slug: "dsa".to_owned(),
        title: "DSA".to_owned(),
        description: String::new(),
        tags: vec![],
        estimated_reading_minutes: None,
        order: None,
        category_path: vec!["learn".to_owned()],
        entries: vec![
            problem("root-problem"),
            chapter(
                "basics",
                vec![
                    lesson("notes"),
                    problem("p1"),
                    problem("p2"),
                    problem("p3"),
                    problem("p4"),
                    problem("p5"),
                    problem("p6"),
                ],
            ),
            chapter("arrays", vec![problem("a1"), problem("a2"), problem("a3")]),
        ],
    }
}

#[test]
fn counts_the_problems_of_the_current_chapter_and_locates_the_path() {
    let (problems, at) = chapter_problems(&problem_book(), "learn/dsa/basics/p5").unwrap();
    assert_eq!(problems.len(), 6, "six problems, the prose note excluded");
    assert_eq!(at, 4, "p5 is the fifth — PROBLEM 5 / 6");
}

#[test]
fn a_prose_lesson_is_neither_counted_nor_located() {
    let (problems, _) = chapter_problems(&problem_book(), "learn/dsa/basics/p1").unwrap();
    assert!(!problems.iter().any(|p| p.ends_with("/notes")));
    assert!(chapter_problems(&problem_book(), "learn/dsa/basics/notes").is_none());
}

#[test]
fn a_sibling_chapters_problems_do_not_bleed_in() {
    let (problems, at) = chapter_problems(&problem_book(), "learn/dsa/arrays/a2").unwrap();
    assert_eq!(problems.len(), 3, "arrays has three, not nine");
    assert_eq!(at, 1);
}

/// A book that never introduced chapters still counts: the book prefix IS the parent.
#[test]
fn book_root_problems_count_against_each_other() {
    let (problems, at) = chapter_problems(&problem_book(), "learn/dsa/root-problem").unwrap();
    assert_eq!(problems, vec!["learn/dsa/root-problem".to_owned()]);
    assert_eq!(at, 0);
}

#[test]
fn an_unknown_path_has_no_position() {
    assert!(chapter_problems(&problem_book(), "learn/dsa/basics/nope").is_none());
    assert!(chapter_problems(&problem_book(), "bare").is_none());
}

#[test]
fn a_chapter_with_no_problems_yields_nothing() {
    let prose_only = BookDto {
        entries: vec![chapter("reading", vec![lesson("one"), lesson("two")])],
        ..problem_book()
    };
    assert!(chapter_problems(&prose_only, "learn/dsa/reading/one").is_none());
}

/// The shape the real content uses: `problems/<slug>/<slug>.md`, one chapter per problem. Scoping
/// on the raw parent would read "1 / 1" on every page; the counter has to flatten these the way
/// the sidebar already does.
#[test]
fn problems_in_a_chapter_of_their_own_still_count_together() {
    let own_dirs = BookDto {
        entries: vec![chapter(
            "problems",
            vec![
                chapter("input-output", vec![problem("input-output")]),
                chapter("if-else-if", vec![problem("if-else-if")]),
                chapter("switch-case", vec![problem("switch-case")]),
            ],
        )],
        ..problem_book()
    };
    let (problems, at) = chapter_problems(&own_dirs, "learn/dsa/problems/if-else-if/if-else-if").unwrap();
    assert_eq!(problems.len(), 3, "the three siblings, not one apiece");
    assert_eq!(at, 1);
}
