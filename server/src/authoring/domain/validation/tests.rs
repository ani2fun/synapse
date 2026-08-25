//! Tests for the edit guardrails and the drift fingerprint.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

const WITH_FENCE: &str = "---\ntitle: Thinking in Tradeoffs\nsummary: A lede\n---\n\nBody prose.\n";
const NO_FENCE: &str = "# Thinking in Tradeoffs\n\nBody prose.\n";

// ── normalise ────────────────────────────────────────────────────────────────

#[test]
fn windows_line_endings_collapse_so_a_paste_is_not_a_whole_file_diff() {
    assert_eq!(normalise("a\r\nb\r\n"), "a\nb\n");
    assert_eq!(normalise("a\rb\r"), "a\nb\n");
}

#[test]
fn the_file_ends_with_exactly_one_newline() {
    assert_eq!(normalise("a"), "a\n");
    assert_eq!(normalise("a\n"), "a\n");
    assert_eq!(normalise("a\n\n\n\n"), "a\n");
}

#[test]
fn interior_blank_lines_are_left_alone() {
    // Paragraph breaks are meaning, not whitespace noise.
    assert_eq!(normalise("a\n\nb\n"), "a\n\nb\n");
}

#[test]
fn an_empty_source_stays_empty_rather_than_becoming_a_newline() {
    assert_eq!(normalise(""), "");
}

// ── fingerprint ──────────────────────────────────────────────────────────────

#[test]
fn the_fingerprint_is_stable_and_sixteen_hex_digits() {
    let one = fingerprint(WITH_FENCE);
    assert_eq!(one, fingerprint(WITH_FENCE));
    assert_eq!(one.len(), 16);
    assert!(one.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn any_real_change_moves_the_fingerprint() {
    assert_ne!(
        fingerprint(WITH_FENCE),
        fingerprint(&WITH_FENCE.replace("prose", "text"))
    );
    assert_ne!(fingerprint(""), fingerprint("a"));
}

#[test]
fn line_ending_churn_alone_does_not_move_the_fingerprint() {
    // Otherwise every Windows contributor would be told the file moved under them.
    assert_eq!(fingerprint("a\nb\n"), fingerprint("a\r\nb\r\n"));
    assert_eq!(fingerprint("a\nb"), fingerprint("a\nb\n"));
}

// ── validate ─────────────────────────────────────────────────────────────────

#[test]
fn an_unchanged_file_validates_to_itself() {
    assert_eq!(validate(WITH_FENCE, WITH_FENCE).unwrap(), WITH_FENCE);
}

#[test]
fn a_normal_edit_passes_and_comes_back_normalised() {
    let edited = WITH_FENCE.replace("Body prose.", "Sharper body prose.");
    let out = validate(WITH_FENCE, &edited.replace('\n', "\r\n")).unwrap();
    assert!(out.contains("Sharper body prose."));
    assert!(!out.contains('\r'));
    assert!(out.ends_with('\n'));
}

#[test]
fn an_empty_or_blank_proposal_is_refused() {
    assert_eq!(validate(WITH_FENCE, "").unwrap_err(), InvalidEdit::Empty);
    assert_eq!(
        validate(WITH_FENCE, "   \n\n\t\n").unwrap_err(),
        InvalidEdit::Empty
    );
}

#[test]
fn an_oversized_proposal_is_refused_before_it_reaches_the_forge() {
    let huge = format!("{WITH_FENCE}{}", "x".repeat(MAX_SOURCE_BYTES));
    match validate(WITH_FENCE, &huge).unwrap_err() {
        InvalidEdit::TooLarge { cap, bytes } => {
            assert_eq!(cap, MAX_SOURCE_BYTES);
            assert!(bytes > cap);
        }
        other => panic!("expected TooLarge, got {other:?}"),
    }
}

#[test]
fn losing_the_frontmatter_fence_is_refused() {
    // The select-all-and-paste accident: the page keeps rendering, but its title, summary and
    // Open Graph tags all silently change.
    assert_eq!(
        validate(WITH_FENCE, "# Thinking in Tradeoffs\n\nBody prose.\n").unwrap_err(),
        InvalidEdit::FrontmatterLost
    );
}

#[test]
fn an_unclosed_fence_counts_as_losing_it() {
    assert_eq!(
        validate(WITH_FENCE, "---\ntitle: Still Here\n\nBody prose.\n").unwrap_err(),
        InvalidEdit::FrontmatterLost
    );
}

#[test]
fn a_lesson_that_never_had_a_fence_is_not_required_to_grow_one() {
    let edited = NO_FENCE.replace("Body prose.", "Better prose.");
    assert!(validate(NO_FENCE, &edited).is_ok());
}

#[test]
fn adding_a_fence_to_a_lesson_that_had_none_is_allowed() {
    assert!(validate(NO_FENCE, WITH_FENCE).is_ok());
}

#[test]
fn a_proposal_with_no_title_left_is_refused() {
    assert_eq!(
        validate(WITH_FENCE, "---\nsummary: A lede\n---\n\nBody with no heading.\n").unwrap_err(),
        InvalidEdit::TitleLost
    );
    assert_eq!(
        validate(NO_FENCE, "Body with no heading.\n").unwrap_err(),
        InvalidEdit::TitleLost
    );
}

#[test]
fn a_body_h1_satisfies_the_title_rule_when_the_fence_has_no_title() {
    assert!(validate(WITH_FENCE, "---\nkind: prose\n---\n\n# A Heading\n\nBody.\n").is_ok());
}
