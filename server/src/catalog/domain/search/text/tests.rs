//! Markdown in, terms out — and prose a person can read.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

#[test]
fn markdown_syntax_dissolves_into_its_words() {
    assert_eq!(tokenize("## Window Functions"), vec!["window", "functions"]);
    assert_eq!(tokenize("**bold** and _italic_"), vec!["bold", "and", "italic"]);
    // Link text is what a reader searches for; the URL contributes its own words, which is noise
    // we accept rather than parse markdown to avoid.
    assert_eq!(
        tokenize("[two sum](/dsa/two-sum)"),
        vec!["two", "sum", "dsa", "two", "sum"]
    );
}

/// Single characters are kept on purpose: this library teaches `C` and `R`, and a length floor
/// would lose exactly the rare terms search is for.
#[test]
fn a_single_character_is_a_term() {
    assert_eq!(tokenize("the C language"), vec!["the", "c", "language"]);
}

#[test]
fn case_folds_and_non_ascii_survives() {
    assert_eq!(tokenize("Naïve CACHING"), vec!["naïve", "caching"]);
    assert_eq!(tokenize("日本語 text"), vec!["日本語", "text"]);
}

#[test]
fn an_absurdly_long_token_is_dropped() {
    let blob = "a".repeat(MAX_TOKEN + 1);
    assert!(tokenize(&blob).is_empty(), "base64 debris is not a search term");
    assert_eq!(
        tokenize(&"a".repeat(MAX_TOKEN)).len(),
        1,
        "the cap itself is kept"
    );
}

/// The trap a naive line scanner falls into: a `#` comment inside a shell example is not a
/// section title.
#[test]
fn a_hash_inside_a_fence_is_not_a_heading() {
    let parts = split("# Real Heading\n\n```bash\n# just a comment\nls -la\n```\n\nProse.\n");
    assert!(parts.headings.contains("Real Heading"));
    assert!(!parts.headings.contains("just a comment"));
    assert!(parts.code.contains("just a comment"));
    assert!(parts.prose.contains("Prose."));
}

/// Only the same marker closes a fence, so a backtick fence quoted inside a tilde fence does not
/// desync the scanner and spill code into prose for the rest of the file.
#[test]
fn a_mismatched_fence_marker_does_not_desync() {
    let parts = split("~~~\n```\nstill code\n~~~\n\nBack to prose.\n");
    assert!(parts.code.contains("still code"));
    assert!(parts.prose.contains("Back to prose."));
    assert!(!parts.prose.contains("still code"));
}

#[test]
fn code_is_separated_from_the_prose_it_sits_in() {
    let parts = split("Before.\n\n```sql\nSELECT 1;\n```\n\nAfter.\n");
    assert!(parts.code.contains("SELECT 1;"));
    assert!(parts.prose.contains("Before.") && parts.prose.contains("After."));
    assert!(!parts.prose.contains("SELECT"), "a snippet must read as prose");
}

#[test]
fn flatten_strips_list_markers_and_collapses_whitespace() {
    let flat = flatten("- first item\n\n\n2. second    item\n> quoted\n");
    assert_eq!(flat, "first item second item quoted");
}

#[test]
fn flatten_of_nothing_is_nothing() {
    assert_eq!(flatten(""), "");
    assert_eq!(flatten("\n\n   \n"), "");
}
