//! `GitHubFetcher` against a loopback mock (wiremock) — the two-call choreography and the error
//! shapes an admin ends up reading, with no network.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

const REPO: &str = "ani2fun/java-guide";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn fetcher(base: &str) -> GitHubFetcher {
    GitHubFetcher::at(base, "ghp_token")
}

async fn head_returns(server: &MockServer, body: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/commits/main")))
        .and(header("accept", "application/vnd.github.sha"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

/// The cheap check is the point: at a 60s cadence the usual answer is "nothing moved", and the
/// archive must not be pulled to discover that.
#[tokio::test]
async fn an_unchanged_head_never_asks_for_the_archive() {
    let server = MockServer::start().await;
    head_returns(&server, SHA).await;
    // No tarball mock is mounted: reaching for it would 404 and fail the test.

    let result = fetcher(&server.uri()).fetch(REPO, "main", Some(SHA)).await;

    assert_eq!(result.unwrap(), Fetched::Unchanged);
}

#[tokio::test]
async fn a_moved_head_pulls_the_archive_for_that_exact_commit() {
    let server = MockServer::start().await;
    head_returns(&server, &format!("{SHA}\n")).await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/tarball/{SHA}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"gzip-bytes".to_vec()))
        .mount(&server)
        .await;

    let fetched = fetcher(&server.uri())
        .fetch(REPO, "main", Some("an-older-sha"))
        .await
        .unwrap();

    match fetched {
        Fetched::Archive { sha, bytes } => {
            assert_eq!(sha, SHA, "the trailing newline is trimmed");
            assert_eq!(bytes, b"gzip-bytes");
        }
        Fetched::Unchanged => panic!("expected an archive"),
    }
}

#[tokio::test]
async fn a_first_fetch_has_no_known_sha_and_still_pulls() {
    let server = MockServer::start().await;
    head_returns(&server, SHA).await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/tarball/{SHA}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"x".to_vec()))
        .mount(&server)
        .await;

    let fetched = fetcher(&server.uri()).fetch(REPO, "main", None).await.unwrap();
    assert!(matches!(fetched, Fetched::Archive { .. }));
}

#[tokio::test]
async fn a_missing_repository_names_itself_in_the_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/commits/main")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let error = fetcher(&server.uri())
        .fetch(REPO, "main", None)
        .await
        .unwrap_err();

    // This string lands in the source row's last_error, so it has to say WHICH repo.
    assert!(error.to_string().contains(REPO), "{error}");
    assert!(matches!(error, FetchError::NotFound(_)), "{error:?}");
}

/// A rate limit must be waited out, not retried — so it is a distinct variant carrying its reset.
#[tokio::test]
async fn an_exhausted_rate_limit_reports_how_long_to_wait() {
    let server = MockServer::start().await;
    let reset = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 90;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/commits/main")))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", reset.to_string().as_str()),
        )
        .mount(&server)
        .await;

    let error = fetcher(&server.uri())
        .fetch(REPO, "main", None)
        .await
        .unwrap_err();

    match error {
        FetchError::RateLimited { seconds } => assert!((80..=90).contains(&seconds), "{seconds}"),
        other => panic!("expected a rate limit, got {other:?}"),
    }
}

/// A 403 that is NOT a rate limit is a scope problem, and the hint should say so.
#[tokio::test]
async fn a_forbidden_response_points_at_the_token_scope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/commits/main")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let error = fetcher(&server.uri())
        .fetch(REPO, "main", None)
        .await
        .unwrap_err();

    assert!(matches!(error, FetchError::Denied(_)), "{error:?}");
    assert!(error.to_string().contains("scope"), "{error}");
}

#[tokio::test]
async fn an_empty_head_is_malformed_rather_than_an_empty_sha() {
    let server = MockServer::start().await;
    head_returns(&server, "   ").await;

    let error = fetcher(&server.uri())
        .fetch(REPO, "main", None)
        .await
        .unwrap_err();

    assert!(matches!(error, FetchError::Malformed(_)), "{error:?}");
}
