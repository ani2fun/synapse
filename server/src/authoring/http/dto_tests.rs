//! Tests for the error→status mapping and the request projection.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::TimeZone;

use crate::authoring::domain::{EditRequestId, EditRequestState, PullRequestRef};

use super::*;

fn at(day: u32) -> chrono::DateTime<chrono::Utc> {
    chrono::Utc.with_ymd_and_hms(2026, 7, day, 12, 0, 0).unwrap()
}

fn row() -> EditRequest {
    EditRequest {
        id: EditRequestId(uuid::Uuid::nil()),
        username: "ani2fun".to_owned(),
        lesson_path: "book/chapter/lesson".to_owned(),
        file_path: "01-book/02-chapter/03-lesson.md".to_owned(),
        repo: "ani2fun/synapse-content".to_owned(),
        branch: "edit/ani2fun/book/chapter/lesson".to_owned(),
        attempt: 1,
        pull_request: Some(PullRequestRef {
            number: 42,
            url: "https://github.com/ani2fun/synapse-content/pull/42".to_owned(),
        }),
        state: EditRequestState::Open,
        commits: 2,
        created_at: at(20),
        updated_at: at(21),
    }
}

#[test]
fn a_request_projects_its_branch_pull_request_and_history() {
    let dto = to_request(&row(), false, "github");
    assert_eq!(dto.branch, "edit/ani2fun/book/chapter/lesson");
    assert_eq!(dto.state, "open");
    assert_eq!(dto.pr_number, Some(42));
    assert_eq!(dto.commits, 2);
    assert_eq!(dto.mode, "github");
    assert!(dto.created_at.ends_with('Z') && dto.updated_at.ends_with('Z'));
}

#[test]
fn a_dry_run_row_carries_no_pull_request_fields() {
    let mut dry = row();
    dry.pull_request = None;
    let dto = to_request(&dry, false, "dry-run");
    assert_eq!(dto.pr_number, None);
    assert_eq!(dto.pr_url, None);
    assert_eq!(
        dto.mode, "dry-run",
        "the client must be able to say nothing was opened"
    );
}

#[test]
fn every_error_maps_to_the_status_its_client_branches_on() {
    let cases = [
        (AuthoringError::NotEditable("x".to_owned()), StatusCode::NOT_FOUND),
        (AuthoringError::RequiresSignIn, StatusCode::UNAUTHORIZED),
        (AuthoringError::NotAllowed("x".to_owned()), StatusCode::FORBIDDEN),
        (
            AuthoringError::Rejected(InvalidEdit::FrontmatterLost),
            StatusCode::BAD_REQUEST,
        ),
        (AuthoringError::NoChange, StatusCode::BAD_REQUEST),
        (AuthoringError::SourceMoved("x".to_owned()), StatusCode::CONFLICT),
        (
            AuthoringError::ForgeUnavailable("x".to_owned()),
            StatusCode::BAD_GATEWAY,
        ),
        (
            AuthoringError::ContentUnreadable("x".to_owned()),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
        (
            AuthoringError::StoreFailed("x".to_owned()),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ];
    for (error, expected) in cases {
        let (status, body) = to_error(&error);
        assert_eq!(status, expected, "{error:?}");
        assert!(body.detail.is_some(), "the cause is never swallowed: {error:?}");
    }
}

#[test]
fn the_errors_a_contributor_hits_mid_edit_say_what_to_do_next() {
    for error in [
        AuthoringError::SourceMoved("x".to_owned()),
        AuthoringError::ForgeUnavailable("x".to_owned()),
        AuthoringError::NotAllowed("x".to_owned()),
        AuthoringError::RequiresSignIn,
    ] {
        assert!(to_error(&error).1.hint.is_some(), "{error:?} needs a next step");
    }
}

/// What carrying `InvalidEdit` whole is FOR. Four rules, four remedies — a flattened string could
/// only ever have produced one apology for all of them, and this fails if a new rule is added
/// without giving the contributor something to do about it.
#[test]
fn each_broken_rule_gets_its_own_remedy() {
    let rules = [
        InvalidEdit::Empty,
        InvalidEdit::TooLarge {
            bytes: 400_000,
            cap: 262_144,
        },
        InvalidEdit::FrontmatterLost,
        InvalidEdit::TitleLost,
    ];
    let mut hints = std::collections::HashSet::new();
    for rule in rules {
        let (status, body) = to_error(&AuthoringError::Rejected(rule.clone()));
        assert_eq!(status, StatusCode::BAD_REQUEST, "{rule:?}");
        let hint = body
            .hint
            .clone()
            .unwrap_or_else(|| panic!("{rule:?} needs a remedy"));
        assert!(hints.insert(hint), "{rule:?} repeats another rule's remedy");
        // The rule's own wording still rides `detail`, so a bug report carries the numbers.
        assert!(
            body.detail
                .clone()
                .unwrap_or_default()
                .contains(&rule.to_string())
        );
    }
}
