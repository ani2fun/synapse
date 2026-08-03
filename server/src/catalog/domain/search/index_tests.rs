//! Building the index: what goes in, and what each field is worth.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

pub(super) fn doc(title: &str, body: &str) -> DocInput {
    DocInput {
        title: title.to_owned(),
        breadcrumb: vec!["Learn".to_owned(), "A Book".to_owned()],
        url: format!("learn/a-book/{}", title.to_lowercase().replace(' ', "-")),
        kind: DocKind::Lesson,
        book_slug: "a-book".to_owned(),
        source_id: "spine".to_owned(),
        summary: None,
        body: body.to_owned(),
    }
}

pub(super) fn index_of(docs: Vec<DocInput>) -> SearchIndex {
    let mut builder = SearchIndex::builder();
    for input in docs {
        builder.add(input);
    }
    builder.build()
}

#[test]
fn an_empty_index_answers_nothing_rather_than_panicking() {
    let index = SearchIndex::default();
    assert!(index.is_empty());
    assert_eq!(index.len(), 0);
    assert!(index.search("anything", 10).is_empty());
}

#[test]
fn a_document_is_findable_by_a_word_only_its_body_contains() {
    let index = index_of(vec![doc(
        "Storage Engines",
        "An LSM tree writes a tombstone on delete.",
    )]);
    let hits = index.search("tombstone", 10);
    assert_eq!(hits.len(), 1, "the whole point: prose is searchable");
    assert_eq!(hits[0].title, "Storage Engines");
}

#[test]
fn the_hit_carries_what_a_result_row_needs() {
    let index = index_of(vec![doc("Storage Engines", "A tombstone marks a deletion.")]);
    let hit = index.search("tombstone", 1).into_iter().next().expect("a hit");
    assert_eq!(hit.url, "learn/a-book/storage-engines");
    assert_eq!(hit.breadcrumb, vec!["Learn".to_owned(), "A Book".to_owned()]);
    assert_eq!(hit.book_slug, "a-book");
    assert_eq!(hit.source_id, "spine");
    assert_eq!(hit.kind, DocKind::Lesson);
    assert!(hit.snippet.iter().any(|s| s.marked), "a hit quotes what matched");
}

/// Code is indexed because this is a programming library — a reader searching a SQL clause
/// expects the examples to count — but it is deliberately weighted below prose.
#[test]
fn fenced_code_is_searchable() {
    let index = index_of(vec![doc(
        "Frames",
        "Some prose.\n\n```sql\nSELECT x OVER (PARTITION BY id)\n```\n",
    )]);
    assert_eq!(index.search("partition", 10).len(), 1);
}

/// A term in a title says the document IS about it; the same term in a body may be an aside.
///
/// The body-only document repeats the term FOUR times against the titled one's single mention, so
/// raw frequency would pick the wrong one — only the title boost can invert that, which is what
/// makes this a test of the boost rather than of tie-breaking.
#[test]
fn a_title_hit_outranks_a_repeated_body_hit() {
    let index = index_of(vec![
        doc("Passing Mention", "indexing indexing indexing indexing"),
        doc("Indexing", "Prose that never repeats it."),
    ]);
    let hits = index.search("indexing", 10);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].title, "Indexing", "the titled lesson leads");
}

/// A satellite's breadcrumb is decided by its registration, not by its own walk — so whatever the
/// caller supplies is what a reader sees, and it must survive to the hit unchanged.
#[test]
fn the_breadcrumb_the_caller_supplies_is_the_one_returned() {
    let mut input = doc("Streams", "Prose about backpressure.");
    input.breadcrumb = vec!["Programming Languages".to_owned(), "Java".to_owned()];
    let index = index_of(vec![input]);
    let hit = index.search("backpressure", 1).into_iter().next().expect("a hit");
    assert_eq!(
        hit.breadcrumb,
        vec!["Programming Languages".to_owned(), "Java".to_owned()]
    );
}

#[test]
fn an_editorial_keeps_its_kind_so_the_surface_can_warn() {
    let mut input = doc("Two Sum", "The two-pointer walk is the intended solution.");
    input.kind = DocKind::Editorial;
    let index = index_of(vec![input]);
    let hit = index.search("two pointer", 1).into_iter().next().expect("a hit");
    assert_eq!(hit.kind, DocKind::Editorial, "a solution must be markable as one");
}

#[test]
fn a_frontmatter_summary_is_searchable() {
    let mut input = doc("Latency", "Body without the word.");
    input.summary = Some("Percentiles and tail latency".to_owned());
    let index = index_of(vec![input]);
    assert_eq!(index.search("percentiles", 10).len(), 1);
}

/// Long documents must not win merely by being long — this corpus runs from 2 KB lessons to a
/// 181 KB chapter.
///
/// Neither title contains the term and the LONG document mentions it more often, so raw term
/// frequency would pick the wrong one. Only length normalisation can flip it, which is what makes
/// this a test of length normalisation rather than of the title boost.
#[test]
fn a_long_document_does_not_beat_a_focused_one_on_repetition_alone() {
    let padded = format!(
        "{} sharding sharding sharding {}",
        "filler ".repeat(400),
        "more ".repeat(400)
    );
    let index = index_of(vec![
        doc("Long Chapter", &padded),
        doc("Short Note", "Sharding splits data across nodes."),
    ]);
    let hits = index.search("sharding", 10);
    assert_eq!(hits[0].title, "Short Note", "focus beats bulk");
}
