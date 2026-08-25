//! Tests for branch derivation: the readable shape, the attempt suffix, git's ref rules, and
//! what happens to a path too long to keep whole.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

const REAL_PAGE: &str = "system-design-from-first-principles/foundations/thinking-in-tradeoffs";

// ── the shape ────────────────────────────────────────────────────────────────

#[test]
fn the_first_attempt_is_the_plain_readable_name() {
    assert_eq!(
        branch_for("ani2fun", REAL_PAGE, 1),
        "edit/ani2fun/system-design-from-first-principles/foundations/thinking-in-tradeoffs"
    );
}

#[test]
fn later_attempts_carry_a_numeric_suffix() {
    assert_eq!(
        branch_for("ani2fun", REAL_PAGE, 2),
        "edit/ani2fun/system-design-from-first-principles/foundations/thinking-in-tradeoffs-2"
    );
    assert!(branch_for("ani2fun", REAL_PAGE, 11).ends_with("-11"));
}

#[test]
fn attempt_zero_reads_as_the_first_attempt() {
    // Nothing produces 0, but a stored row is data — it must not grow a "-0" suffix.
    assert_eq!(
        branch_for("ani2fun", REAL_PAGE, 0),
        branch_for("ani2fun", REAL_PAGE, 1)
    );
}

#[test]
fn the_same_inputs_always_derive_the_same_branch() {
    // The reuse rule depends on this: the branch is derived per submit, never looked up.
    assert_eq!(
        branch_for("Ani2Fun", REAL_PAGE, 1),
        branch_for("ani2fun", REAL_PAGE, 1)
    );
}

// ── sanitising ───────────────────────────────────────────────────────────────

#[test]
fn usernames_are_lowercased_and_stripped_to_ref_safe_characters() {
    assert_eq!(
        branch_for("Ada.Lovelace", "book/one", 1),
        "edit/ada-lovelace/book/one"
    );
    assert_eq!(
        branch_for("grace@navy.mil", "book/one", 1),
        "edit/grace-navy-mil/book/one"
    );
    assert_eq!(branch_for("keep_this", "book/one", 1), "edit/keep_this/book/one");
}

#[test]
fn a_username_with_nothing_usable_falls_back_rather_than_producing_an_empty_segment() {
    assert_eq!(branch_for("!!!", "book/one", 1), "edit/contributor/book/one");
    assert_eq!(branch_for("", "book/one", 1), "edit/contributor/book/one");
}

#[test]
fn an_empty_path_falls_back_rather_than_ending_the_ref_in_a_slash() {
    assert_eq!(branch_for("ada", "", 1), "edit/ada/lesson");
    assert_eq!(branch_for("ada", "///", 1), "edit/ada/lesson");
}

#[test]
fn hostile_path_input_cannot_produce_an_illegal_ref() {
    // None of these reach production — the catalog only resolves slug-like paths — but the
    // branch name is the wrong place to rely on that.
    for path in [
        "../../etc/passwd",
        "book/../../..",
        "book/one~two",
        "book/one two",
        "book/.hidden/lesson",
        "book/one.lock",
        "book//one",
        "book/one@{upstream}",
    ] {
        let branch = branch_for("ada", path, 1);
        assert!(
            is_valid_ref(&branch),
            "'{path}' produced an illegal ref: {branch}"
        );
    }
}

// ── length ───────────────────────────────────────────────────────────────────

#[test]
fn a_path_too_long_to_keep_whole_keeps_its_tail_and_gains_a_digest() {
    let deep = (0..12)
        .map(|i| format!("chapter-{i}-with-a-deliberately-long-slug-segment"))
        .collect::<Vec<_>>()
        .join("/");
    let path = format!("{deep}/thinking-in-tradeoffs");
    let branch = branch_for("ani2fun", &path, 1);

    assert!(is_valid_ref(&branch), "{branch}");
    assert!(branch.len() <= MAX_BRANCH_LEN, "{} chars", branch.len());
    assert!(branch.starts_with("edit/ani2fun/"));
    assert!(
        branch.contains("thinking-in-tradeoffs"),
        "the lesson's own name survives"
    );
}

#[test]
fn two_long_paths_sharing_a_tail_still_get_different_branches() {
    let prefix = "a-very-long-book-slug-that-eats-the-whole-branch-budget-on-its-own";
    let deep = |book: &str| {
        format!("{book}/{prefix}/{prefix}/{prefix}/chapter-with-a-long-name/thinking-in-tradeoffs")
    };
    let one = branch_for("ada", &deep("book-one"), 1);
    let two = branch_for("ada", &deep("book-two"), 1);

    assert!(one.len() <= MAX_BRANCH_LEN && two.len() <= MAX_BRANCH_LEN);
    assert_ne!(one, two, "the digest must keep them distinct");
}

#[test]
fn an_absurdly_long_username_cannot_starve_the_page_path() {
    let branch = branch_for(&"a".repeat(500), REAL_PAGE, 1);
    assert!(is_valid_ref(&branch), "{branch}");
    assert!(branch.contains("thinking-in-tradeoffs"));
}

// ── the validator itself ─────────────────────────────────────────────────────

#[test]
fn is_valid_ref_rejects_what_git_rejects() {
    assert!(is_valid_ref("edit/ada/book/lesson"));
    for bad in [
        "",
        "@",
        "/leading",
        "trailing/",
        "double//slash",
        "dot..dot",
        "trailing.",
        "some.lock",
        "has space",
        "has~tilde",
        "has^caret",
        "has:colon",
        "has?question",
        "has*star",
        "has[bracket",
        "has\\backslash",
        "at@{brace",
        "edit/.hidden/lesson",
    ] {
        assert!(!is_valid_ref(bad), "'{bad}' should be rejected");
    }
}
