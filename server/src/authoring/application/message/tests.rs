//! Tests for what a reviewer reads.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

const PAGE: &str = "system-design-from-first-principles/foundations/thinking-in-tradeoffs";

#[test]
fn the_commit_names_the_requester_rather_than_authoring_as_them() {
    let message = commit_message("book/lesson", "ani2fun", Some("Fixed a typo in the intro."));
    assert!(message.starts_with("content: edit book/lesson\n\n"));
    assert!(message.contains("Fixed a typo in the intro."));
    assert!(message.contains("Requested by @ani2fun via Synapse."));
    // The contributor's address is deliberately nowhere in here.
    assert!(!message.contains('@') || !message.contains('<'));
}

#[test]
fn a_missing_or_blank_summary_leaves_no_empty_paragraph() {
    for summary in [None, Some(""), Some("   \n ")] {
        let message = commit_message("book/lesson", "ada", summary);
        assert_eq!(
            message,
            "content: edit book/lesson\n\nRequested by @ada via Synapse.\n"
        );
    }
}

#[test]
fn a_deep_path_does_not_produce_an_unreadable_subject() {
    let deep = (0..20)
        .map(|i| format!("segment-{i}"))
        .collect::<Vec<_>>()
        .join("/");
    let subject = commit_message(&deep, "ada", None)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    assert!(subject.chars().count() <= SUBJECT_LIMIT, "{subject}");
    assert!(subject.ends_with('…'));
}

#[test]
fn the_pull_request_title_names_the_page() {
    assert_eq!(pull_request_title("book/lesson"), "Content edit: book/lesson");
    assert!(pull_request_title(&"x".repeat(400)).chars().count() <= TITLE_LIMIT);
}

#[test]
fn the_body_links_the_live_page_so_prose_can_be_reviewed_in_place() {
    let body = pull_request_body(
        "https://synapse.kakde.eu",
        PAGE,
        "system-design-from-first-principles/01-foundations/01-thinking-in-tradeoffs.md",
        "ani2fun",
        Some("Clarified the CAP paragraph."),
    );
    assert!(body.contains(&format!("https://synapse.kakde.eu/synapse/{PAGE}")));
    assert!(
        body.contains("`system-design-from-first-principles/01-foundations/01-thinking-in-tradeoffs.md`")
    );
    assert!(body.contains("@ani2fun"));
    assert!(body.contains("Clarified the CAP paragraph."));
}

#[test]
fn a_trailing_slash_on_the_site_url_does_not_double_up() {
    let body = pull_request_body("https://synapse.kakde.eu/", "book/lesson", "b/l.md", "ada", None);
    assert!(body.contains("https://synapse.kakde.eu/synapse/book/lesson"));
    assert!(!body.contains("eu//synapse"));
}

#[test]
fn a_body_without_a_summary_says_so_rather_than_showing_a_gap() {
    let body = pull_request_body("https://s.example", "book/lesson", "b/l.md", "ada", None);
    assert!(body.contains(NO_SUMMARY));
}
