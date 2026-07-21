//! Tests for the catalog frontmatter vocabulary: title/summary/kind extraction and their
//! fallback and blank-degrades-to-`None` rules.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

// ── extract_title ─────────────────────────────────────────────────────────────

#[test]
fn title_prefers_frontmatter_over_h1() {
    let src = "---\ntitle: From Fence\n---\n# From Heading\nbody";
    assert_eq!(extract_title(src, "Fallback"), "From Fence");
}

#[test]
fn title_falls_back_to_first_h1() {
    assert_eq!(
        extract_title("---\nkind: problem\n---\n# From Heading\nbody", "Fallback"),
        "From Heading"
    );
    assert_eq!(extract_title("# From Heading\nbody", "Fallback"), "From Heading");
}

#[test]
fn title_falls_back_to_caller_fallback() {
    assert_eq!(extract_title("just prose", "Fallback"), "Fallback");
}

#[test]
fn unterminated_fence_is_plain_body() {
    // No closing --- → the fence never existed; the # line is a real heading.
    assert_eq!(
        extract_title("---\ntitle: Ghost\n# Real Heading\nbody", "Fallback"),
        "Real Heading"
    );
}

// ── extract_essential ─────────────────────────────────────────────────────────

#[test]
fn essential_true_false_absent_malformed() {
    assert_eq!(extract_essential("---\nessential: true\n---\nbody"), Some(true));
    assert_eq!(extract_essential("---\nessential: false\n---\nbody"), Some(false));
    assert_eq!(extract_essential("---\ntitle: X\n---\nbody"), None);
    assert_eq!(extract_essential("---\nessential: yes\n---\nbody"), None);
    assert_eq!(extract_essential("no fence at all"), None);
}

// ── parse ─────────────────────────────────────────────────────────────────────

#[test]
fn parse_reads_every_field_and_strips_the_fence() {
    let src = "---\ntitle: Two Sum\nsummary: The classic warm-up\nessential: false\nkind: problem\ndifficulty: easy\ntopics: [arrays, \"hash maps\"]\n---\nThe body starts here.";
    let parsed = parse(src, "Fallback");
    assert_eq!(parsed.frontmatter.title, "Two Sum");
    assert_eq!(parsed.frontmatter.summary.as_deref(), Some("The classic warm-up"));
    assert_eq!(parsed.frontmatter.essential, Some(false));
    assert_eq!(parsed.frontmatter.kind.as_deref(), Some("problem"));
    assert_eq!(parsed.frontmatter.difficulty.as_deref(), Some("easy"));
    assert_eq!(
        parsed.frontmatter.topics,
        Some(vec!["arrays".to_owned(), "hash maps".to_owned()])
    );
    assert_eq!(parsed.body, "The body starts here.");
}

#[test]
fn parse_defaults_absent_fields_and_falls_back_title() {
    let parsed = parse("plain body only", "Humanized Name");
    assert_eq!(parsed.frontmatter.title, "Humanized Name");
    assert_eq!(parsed.frontmatter.summary, None);
    assert_eq!(parsed.frontmatter.essential, None);
    assert_eq!(parsed.frontmatter.kind, None);
    assert_eq!(parsed.frontmatter.difficulty, None);
    assert_eq!(parsed.frontmatter.topics, None);
    assert_eq!(parsed.body, "plain body only");
}

#[test]
fn parse_unterminated_fence_keeps_whole_content_as_body() {
    let src = "---\ntitle: Ghost\nbody without terminator";
    let parsed = parse(src, "Fallback");
    assert_eq!(parsed.frontmatter.title, "Fallback");
    assert_eq!(parsed.body, src);
}

#[test]
fn inline_list_strips_quotes_and_blanks() {
    assert_eq!(
        parse_inline_list("[a, 'b c', \"d\", , ]"),
        vec!["a".to_owned(), "b c".to_owned(), "d".to_owned()]
    );
    assert_eq!(parse_inline_list("solo"), vec!["solo".to_owned()]);
}

#[test]
fn fields_drop_empty_values_and_quote_strip() {
    let (fields, body) = fields_and_body("---\nempty:\nquoted: \"v\"\nplain: w\n---\nB");
    assert_eq!(fields.get("empty"), None);
    assert_eq!(fields.get("quoted").map(String::as_str), Some("v"));
    assert_eq!(fields.get("plain").map(String::as_str), Some("w"));
    assert_eq!(body, "B");
}
