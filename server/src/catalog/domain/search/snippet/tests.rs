//! Quoting a document back at the reader, safely.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn terms(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}

fn rendered(segments: &[Segment]) -> String {
    segments.iter().map(|s| s.text.as_str()).collect()
}

fn marked(segments: &[Segment]) -> Vec<&str> {
    segments
        .iter()
        .filter(|s| s.marked)
        .map(|s| s.text.as_str())
        .collect()
}

#[test]
fn the_matched_words_come_back_marked_and_the_rest_does_not() {
    let excerpt = excerpt("A lesson about window functions in SQL.", &terms(&["window"]));
    assert_eq!(marked(&excerpt.segments), vec!["window"]);
    assert!(rendered(&excerpt.segments).contains("window functions"));
}

/// The mark must land on the text as WRITTEN, not lowercased — the reader is being shown their
/// own document.
#[test]
fn a_case_insensitive_match_marks_the_original_casing() {
    let excerpt = excerpt("Idempotency Keys matter.", &terms(&["idempotency"]));
    assert_eq!(marked(&excerpt.segments), vec!["Idempotency"]);
}

#[test]
fn every_occurrence_is_marked_not_just_the_first() {
    let excerpt = excerpt("cache the cache in a cache", &terms(&["cache"]));
    assert_eq!(marked(&excerpt.segments).len(), 3);
}

/// The reason this module returns segments rather than byte offsets: Rust indexes by UTF-8 byte
/// and JavaScript by UTF-16 code unit, so an offset computed here would highlight the wrong span
/// there the moment the prose stops being ASCII.
#[test]
fn multibyte_prose_marks_the_right_span() {
    let excerpt = excerpt("A naïve — really naïve — cache policy", &terms(&["cache"]));
    assert_eq!(marked(&excerpt.segments), vec!["cache"]);
    assert!(rendered(&excerpt.segments).contains("naïve"));
}

/// Slicing a `&str` mid-character panics, and `panic = "deny"` does not catch it. Long multibyte
/// prose is where that would happen.
#[test]
fn a_long_multibyte_document_never_slices_mid_character() {
    let body = format!(
        "{} target {}",
        "日本語のテキスト ".repeat(40),
        "うしろ ".repeat(40)
    );
    let excerpt = excerpt(&body, &terms(&["target"]));
    assert_eq!(marked(&excerpt.segments), vec!["target"]);
    assert!(rendered(&excerpt.segments).chars().count() > 10);
}

#[test]
fn a_long_document_is_truncated_with_ellipses_around_the_match() {
    let body = format!("{} needle {}", "filler ".repeat(200), "trailing ".repeat(200));
    let excerpt = excerpt(&body, &terms(&["needle"]));
    let text = rendered(&excerpt.segments);
    assert!(text.starts_with('…') && text.ends_with('…'));
    assert!(text.contains("needle"));
    assert!(
        text.chars().count() < 300,
        "a quote, not the document: {}",
        text.chars().count()
    );
}

#[test]
fn a_short_document_is_quoted_whole_with_no_ellipsis() {
    let excerpt = excerpt("Short and sweet.", &terms(&["sweet"]));
    assert_eq!(rendered(&excerpt.segments), "Short and sweet.");
}

/// A term present in a heading or in code has nothing to quote from the prose, so the opening
/// stands in rather than the row rendering empty.
#[test]
fn a_term_absent_from_the_prose_falls_back_to_the_opening() {
    let excerpt = excerpt("Some unrelated prose entirely.", &terms(&["partition"]));
    assert!(marked(&excerpt.segments).is_empty());
    assert!(rendered(&excerpt.segments).starts_with("Some unrelated"));
}

#[test]
fn an_empty_document_does_not_panic() {
    let excerpt = excerpt("", &terms(&["anything"]));
    assert_eq!(rendered(&excerpt.segments), "");
}

/// Terms sitting together mean the document is ABOUT the phrase; terms paragraphs apart mean it
/// merely contains both words.
#[test]
fn terms_close_together_score_a_proximity_bonus() {
    let together = excerpt("the window function clause", &terms(&["window", "function"]));
    let apart = format!("window {} function", "filler ".repeat(120));
    let apart = excerpt(&apart, &terms(&["window", "function"]));
    assert!(together.proximity > apart.proximity);
    assert!(
        (apart.proximity - 1.0).abs() < f32::EPSILON,
        "distant terms get no bonus"
    );
}
