//! `GitHubForge` against a loopback mock (wiremock) — the REST choreography the adapter performs,
//! asserted request by request, with no network. The production path is `https://api.github.com`;
//! `GitHubForge::at` points the same code at the mock's origin.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

const REPO: &str = "ani2fun/synapse-content";
const BRANCH: &str = "edit/ada/book/intro";
const FILE: &str = "book/intro.md";

fn forge(base: &str) -> GitHubForge {
    GitHubForge::at(base, REPO, "main", "ghp_token")
}

fn json(body: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

// ── construction ─────────────────────────────────────────────────────────────

#[test]
fn the_owner_is_split_off_the_repo_for_the_head_filter() {
    let f = GitHubForge::new(REPO, "main", "t");
    assert_eq!(f.owner, "ani2fun");
    assert_eq!(f.repo, REPO);
    assert_eq!(f.api_base, "https://api.github.com");
}

#[test]
fn a_repo_without_a_slash_degrades_rather_than_panicking() {
    // Misconfiguration should fail loudly at call time, not at construction.
    assert_eq!(
        GitHubForge::new("synapse-content", "main", "t").owner,
        "synapse-content"
    );
}

#[test]
fn the_mode_is_what_the_client_is_told() {
    assert_eq!(GitHubForge::new("a/b", "main", "t").mode(), "github");
}

// ── commit_file ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn commit_updates_an_existing_file_on_an_existing_branch() {
    let server = MockServer::start().await;

    // The branch exists → no ref creation.
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/git/ref/heads/{BRANCH}")))
        .and(header("authorization", "Bearer ghp_token"))
        .respond_with(json(serde_json::json!({ "object": { "sha": "headsha" } })))
        .mount(&server)
        .await;
    // The file exists → its blob sha comes back for the update.
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/contents/{FILE}")))
        .respond_with(json(serde_json::json!({ "sha": "blobsha" })))
        .mount(&server)
        .await;
    // The PUT carries branch + base64 content + the blob sha, and returns the new commit.
    Mock::given(method("PUT"))
        .and(path(format!("/repos/{REPO}/contents/{FILE}")))
        .respond_with(json(serde_json::json!({ "commit": { "sha": "newcommit" } })))
        .mount(&server)
        .await;

    let sha = forge(&server.uri())
        .commit_file(BRANCH, FILE, "new body", "msg")
        .await
        .unwrap();
    assert_eq!(sha, "newcommit");
}

#[tokio::test]
async fn commit_creates_the_branch_and_a_new_file_when_absent() {
    let server = MockServer::start().await;

    // The branch is missing…
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/git/ref/heads/{BRANCH}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    // …so it is branched off the base's head…
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/git/ref/heads/main")))
        .respond_with(json(serde_json::json!({ "object": { "sha": "mainsha" } })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/repos/{REPO}/git/refs")))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({ "ref": "x" })))
        .mount(&server)
        .await;
    // …the file does not exist (no blob sha)…
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/contents/{FILE}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path(format!("/repos/{REPO}/contents/{FILE}")))
        .respond_with(json(serde_json::json!({ "commit": { "sha": "created" } })))
        .mount(&server)
        .await;

    let sha = forge(&server.uri())
        .commit_file(BRANCH, FILE, "brand new", "msg")
        .await
        .unwrap();
    assert_eq!(sha, "created");
}

#[tokio::test]
async fn a_stale_blob_on_put_is_the_source_moved_signal() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/git/ref/heads/{BRANCH}")))
        .respond_with(json(serde_json::json!({ "object": { "sha": "headsha" } })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/contents/{FILE}")))
        .respond_with(json(serde_json::json!({ "sha": "blobsha" })))
        .mount(&server)
        .await;
    // GitHub's own optimistic-concurrency answer.
    Mock::given(method("PUT"))
        .and(path(format!("/repos/{REPO}/contents/{FILE}")))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({ "message": "is at" })))
        .mount(&server)
        .await;

    let err = forge(&server.uri())
        .commit_file(BRANCH, FILE, "body", "msg")
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AuthoringError::SourceMoved(f) if f == FILE),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_401_names_the_token_scopes_in_the_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/git/ref/heads/{BRANCH}")))
        .respond_with(
            ResponseTemplate::new(401).set_body_json(serde_json::json!({ "message": "Bad credentials" })),
        )
        .mount(&server)
        .await;

    let err = forge(&server.uri())
        .commit_file(BRANCH, FILE, "body", "msg")
        .await
        .unwrap_err();
    match err {
        AuthoringError::ForgeUnavailable(detail) => {
            assert!(
                detail.contains("Bad credentials"),
                "surfaces GitHub's message: {detail}"
            );
            assert!(detail.contains("scopes"), "points at the token scopes: {detail}");
        }
        other => panic!("expected ForgeUnavailable, got {other:?}"),
    }
}

// ── open_pull_request ────────────────────────────────────────────────────────

#[tokio::test]
async fn opening_a_pull_request_returns_its_number_and_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/repos/{REPO}/pulls")))
        .respond_with(json(serde_json::json!({
            "number": 42, "html_url": "https://github.com/ani2fun/synapse-content/pull/42", "state": "open"
        })))
        .mount(&server)
        .await;

    let pr = forge(&server.uri())
        .open_pull_request(BRANCH, "title", "body")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pr.number, 42);
    assert!(pr.url.ends_with("/pull/42"));
}

#[tokio::test]
async fn a_422_reuses_the_pull_request_already_open_for_the_branch() {
    // The idempotence the service leans on: after a store hiccup, a retry re-POSTs and GitHub says
    // "one already exists" (422) — the adapter looks it up and returns it rather than erroring.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/repos/{REPO}/pulls")))
        .respond_with(
            ResponseTemplate::new(422)
                .set_body_json(serde_json::json!({ "message": "A pull request already exists" })),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/pulls")))
        .respond_with(json(serde_json::json!([
            { "number": 7, "html_url": "https://github.com/ani2fun/synapse-content/pull/7", "state": "open" }
        ])))
        .mount(&server)
        .await;

    let pr = forge(&server.uri())
        .open_pull_request(BRANCH, "title", "body")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pr.number, 7, "the already-open PR is reused, not an error");
}

// ── pull_request_state ───────────────────────────────────────────────────────

#[tokio::test]
async fn pull_request_state_maps_every_forge_answer() {
    async fn state_for(body: serde_json::Value, status: u16) -> ForgePrState {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/{REPO}/pulls/1")))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&server)
            .await;
        forge(&server.uri()).pull_request_state(1).await.unwrap()
    }

    assert_eq!(
        state_for(
            serde_json::json!({ "number": 1, "html_url": "u", "merged": true }),
            200
        )
        .await,
        ForgePrState::Merged
    );
    assert_eq!(
        state_for(
            serde_json::json!({ "number": 1, "html_url": "u", "merged_at": "2026-07-24T00:00:00Z" }),
            200
        )
        .await,
        ForgePrState::Merged,
        "merged_at also means merged"
    );
    assert_eq!(
        state_for(
            serde_json::json!({ "number": 1, "html_url": "u", "state": "open" }),
            200
        )
        .await,
        ForgePrState::Open
    );
    assert_eq!(
        state_for(
            serde_json::json!({ "number": 1, "html_url": "u", "state": "closed" }),
            200
        )
        .await,
        ForgePrState::Closed
    );
    assert_eq!(
        state_for(serde_json::json!({ "message": "Not Found" }), 404).await,
        ForgePrState::Missing,
        "a deleted PR is Missing"
    );
}
