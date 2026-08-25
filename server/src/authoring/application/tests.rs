//! Tests for `ProposeEdit` over the in-memory doubles in `service_fakes` — the reuse rule, the
//! drift guard, and the authorisation gate, all without a forge or a database.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod fakes;

use fakes::{
    EDITED, FakeSource, ORIGINAL, PAGE, ani2fun, base, editor, harness, harness_with, harness_without_forge,
    page, revision,
};

use crate::authoring::domain::EditRequestState;
use crate::authoring::domain::validation::InvalidEdit;

use super::*;

// ── the happy path ────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_first_edit_opens_a_branch_and_a_pull_request() {
    let h = harness();
    let proposal = h
        .service
        .propose(Some(&ani2fun()), &page(), EDITED, &base(), Some("Sharper."))
        .await
        .unwrap();

    assert_eq!(proposal.request.location().branch, format!("edit/ani2fun/{PAGE}"));
    assert_eq!(proposal.request.attempt(), 1);
    assert_eq!(proposal.request.commits(), 1);
    assert!(!proposal.reused);
    assert_eq!(proposal.request.state(), EditRequestState::Open);
    assert_eq!(proposal.request.pull_request().map(|pr| pr.number), Some(1));
    assert_eq!(h.forge.commits_on(&proposal.request.location().branch), 1);
    assert_eq!(h.forge.opened_count(), 1);
}

#[tokio::test]
async fn the_committed_content_is_the_proposal_normalised() {
    let h = harness();
    h.service
        .propose(
            Some(&ani2fun()),
            &page(),
            &EDITED.replace('\n', "\r\n"),
            &base(),
            None,
        )
        .await
        .unwrap();

    let commits = h.forge.commits.lock().unwrap();
    let (_, content) = commits.first().unwrap();
    assert_eq!(content, EDITED, "CRLF collapses before it reaches the forge");
}

// ── the reuse rule ────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_second_edit_while_the_pull_request_is_open_adds_a_commit_not_a_pull_request() {
    let h = harness();
    let first = h
        .service
        .propose(Some(&ani2fun()), &page(), EDITED, &base(), None)
        .await
        .unwrap();

    // The editor reloaded and is now working from the committed text.
    h.source.moves_to(EDITED);
    let second = h
        .service
        .propose(
            Some(&ani2fun()),
            &page(),
            &revision("Sharper again."),
            &fingerprint(EDITED),
            None,
        )
        .await
        .unwrap();

    assert!(second.reused);
    assert_eq!(second.request.location().branch, first.request.location().branch);
    assert_eq!(second.request.commits(), 2);
    assert_eq!(h.forge.commits_on(&first.request.location().branch), 2);
    assert_eq!(h.forge.opened_count(), 1, "still one pull request");
    assert_eq!(h.repo.all().len(), 1, "still one change request");
}

#[tokio::test]
async fn an_edit_after_the_pull_request_merged_starts_a_new_branch() {
    let h = harness();
    let first = h
        .service
        .propose(Some(&ani2fun()), &page(), EDITED, &base(), None)
        .await
        .unwrap();
    h.forge.merge(first.request.pull_request().unwrap().number);

    h.source.moves_to(EDITED);
    let second = h
        .service
        .propose(
            Some(&ani2fun()),
            &page(),
            &revision("A later pass."),
            &fingerprint(EDITED),
            None,
        )
        .await
        .unwrap();

    assert!(!second.reused);
    assert_eq!(second.request.attempt(), 2);
    assert_eq!(second.request.location().branch, format!("edit/ani2fun/{PAGE}-2"));
    assert_eq!(h.forge.opened_count(), 2);

    let rows = h.repo.all();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].state(), EditRequestState::Merged, "the first is settled");
    assert_eq!(rows[1].state(), EditRequestState::Open);
}

#[tokio::test]
async fn a_pull_request_that_vanished_from_the_forge_is_not_reused() {
    let h = harness();
    let first = h
        .service
        .propose(Some(&ani2fun()), &page(), EDITED, &base(), None)
        .await
        .unwrap();
    h.forge.forget(first.request.pull_request().unwrap().number);

    h.source.moves_to(EDITED);
    let second = h
        .service
        .propose(
            Some(&ani2fun()),
            &page(),
            &revision("Again."),
            &fingerprint(EDITED),
            None,
        )
        .await
        .unwrap();

    assert!(!second.reused);
    assert_eq!(second.request.attempt(), 2);
}

#[tokio::test]
async fn two_contributors_editing_one_page_get_their_own_branches() {
    let h = harness_with(
        FakeSource::holding(ORIGINAL),
        fakes::FakeForge::default(),
        &["ani2fun", "ada"],
    );
    let one = h
        .service
        .propose(Some(&ani2fun()), &page(), EDITED, &base(), None)
        .await
        .unwrap();
    let two = h
        .service
        .propose(
            Some(&editor("ada")),
            &page(),
            &revision("Ada's take."),
            &base(),
            None,
        )
        .await
        .unwrap();

    assert_eq!(one.request.location().branch, format!("edit/ani2fun/{PAGE}"));
    assert_eq!(two.request.location().branch, format!("edit/ada/{PAGE}"));
    assert_eq!(
        two.request.attempt(),
        1,
        "another person's attempt does not count"
    );
}

// ── the drift guard ───────────────────────────────────────────────────────────

#[tokio::test]
async fn an_edit_against_a_stale_copy_is_refused_and_commits_nothing() {
    let h = harness();
    let stale = base();
    h.source.moves_to(&revision("Someone else got here first."));

    let error = h
        .service
        .propose(Some(&ani2fun()), &page(), EDITED, &stale, None)
        .await
        .unwrap_err();

    assert!(matches!(error, AuthoringError::SourceMoved(path) if path == PAGE));
    assert_eq!(h.forge.commit_count(), 0);
    assert_eq!(h.repo.all().len(), 0);
}

// ── validation ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_proposal_that_loses_the_frontmatter_is_refused_before_the_forge() {
    let h = harness_without_forge(FakeSource::holding(ORIGINAL));
    let error = h
        .service
        .propose(
            Some(&ani2fun()),
            &page(),
            "# Thinking in Tradeoffs\n\nProse.\n",
            &base(),
            None,
        )
        .await
        .unwrap_err();

    // Which rule broke survives as a value, not as prose parsed back out of a string.
    assert_eq!(error, AuthoringError::Rejected(InvalidEdit::FrontmatterLost));
}

#[tokio::test]
async fn a_proposal_identical_to_the_current_file_is_refused() {
    // Otherwise a stray Submit opens an empty pull request for a reviewer to close.
    let h = harness_without_forge(FakeSource::holding(ORIGINAL));
    let error = h
        .service
        .propose(Some(&ani2fun()), &page(), ORIGINAL, &base(), None)
        .await
        .unwrap_err();

    assert_eq!(error, AuthoringError::NoChange);
}

#[tokio::test]
async fn a_path_that_is_not_a_lesson_is_not_editable() {
    let h = harness_without_forge(FakeSource::missing());
    let error = h
        .service
        .propose(Some(&ani2fun()), &page(), EDITED, &base(), None)
        .await
        .unwrap_err();

    assert!(matches!(error, AuthoringError::NotEditable(path) if path == PAGE));
}

// ── the gate ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn anonymous_cannot_propose_and_never_reaches_the_forge() {
    let h = harness_without_forge(FakeSource::holding(ORIGINAL));
    assert_eq!(
        h.service
            .propose(None, &page(), EDITED, &base(), None)
            .await
            .unwrap_err(),
        AuthoringError::RequiresSignIn
    );
    assert!(h.service.source_for(None, &page()).await.is_err());
    assert!(h.service.mine(None).await.is_err());
}

#[tokio::test]
async fn a_signed_in_stranger_cannot_propose() {
    let h = harness_without_forge(FakeSource::holding(ORIGINAL));
    assert_eq!(
        h.service
            .propose(Some(&editor("curious")), &page(), EDITED, &base(), None)
            .await
            .unwrap_err(),
        AuthoringError::NotAllowed("curious".to_owned())
    );
}

#[tokio::test]
async fn may_edit_answers_for_everyone_without_erroring() {
    let h = harness();
    assert!(
        !h.service.may_edit(None).await.unwrap(),
        "anonymous is not an error"
    );
    assert!(h.service.may_edit(Some(&ani2fun())).await.unwrap());
    assert!(!h.service.may_edit(Some(&editor("curious"))).await.unwrap());
}

// ── reads ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_source_comes_back_whole_with_a_fingerprint_that_round_trips() {
    let h = harness();
    let source = h.service.source_for(Some(&ani2fun()), &page()).await.unwrap();

    assert_eq!(source.source, ORIGINAL, "frontmatter fence included");
    assert_eq!(source.lesson_path, PAGE);
    assert_eq!(source.content_version, "sha-abc");
    // The fingerprint it hands out is the one propose accepts.
    assert!(
        h.service
            .propose(Some(&ani2fun()), &page(), EDITED, &source.fingerprint, None)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn mine_lists_only_the_callers_own_requests() {
    let h = harness_with(
        FakeSource::holding(ORIGINAL),
        fakes::FakeForge::default(),
        &["ani2fun", "ada"],
    );
    h.service
        .propose(Some(&ani2fun()), &page(), EDITED, &base(), None)
        .await
        .unwrap();
    h.service
        .propose(
            Some(&editor("ada")),
            &page(),
            &revision("Ada's take."),
            &base(),
            None,
        )
        .await
        .unwrap();

    let mine = h.service.mine(Some(&ani2fun())).await.unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].username(), "ani2fun");
}

// ─────────────────────────────────────────────────────────────────────────────
// ROUTING — which repository a proposal lands in
// ─────────────────────────────────────────────────────────────────────────────

/// A fresh proposal records the repository it targeted, so the row can be followed later.
#[tokio::test]
async fn a_new_proposal_records_the_repository_it_targeted() {
    let h = harness();
    let proposal = h
        .service
        .propose(Some(&ani2fun()), &page(), EDITED, &base(), None)
        .await
        .unwrap();
    assert_eq!(proposal.request.location().repo, "ani2fun/synapse-content");
}

/// A revision follows the repository the branch was OPENED against, not whatever currently serves
/// the page. The two diverge exactly when a book migrates out of the monorepo mid-review — and the
/// open branch is still in the repository it was created in.
#[tokio::test]
async fn a_revision_follows_the_repository_recorded_on_its_row() {
    let h = harness();
    h.service
        .propose(Some(&ani2fun()), &page(), EDITED, &base(), None)
        .await
        .unwrap();
    let opened = h.repo.all().first().cloned().expect("a row was saved");
    assert_eq!(opened.location().repo, "ani2fun/synapse-content");

    let again = h
        .service
        .propose(
            Some(&ani2fun()),
            &page(),
            &revision("Sharper still."),
            &base(),
            None,
        )
        .await
        .unwrap();

    assert!(again.reused, "the open proposal is revised, not re-opened");
    assert_eq!(
        again.request.location().repo,
        opened.location().repo,
        "a revision must stay in the repository its branch lives in"
    );
}
