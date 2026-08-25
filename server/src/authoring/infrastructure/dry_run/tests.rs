//! The dry-run forge's contract is a negative — it accepts every call and writes nothing.
//! CI and e2e run against it, so one real side effect here would reach a real repository.

#![allow(clippy::unwrap_used)]

use super::*;

#[tokio::test]
async fn it_commits_nothing_and_opens_nothing() {
    let forge = DryRunForge::new("ani2fun/synapse-content", "main");
    assert_eq!(forge.mode(), "dry-run");
    assert_eq!(
        forge
            .commit_file("edit/ada/x", "x.md", "body", "subject")
            .await
            .unwrap(),
        "dry-run"
    );
    assert!(
        forge
            .open_pull_request("edit/ada/x", "t", "b")
            .await
            .unwrap()
            .is_none(),
        "a dry run must never look like a real pull request"
    );
}
