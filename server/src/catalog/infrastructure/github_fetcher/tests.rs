//! `GitHubFetcher` against a loopback mock (wiremock) — the two-call choreography and the error
//! shapes an admin ends up reading, with no network.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

const REPO: &str = "ani2fun/java-guide";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// Tests that never pull an archive spool nowhere in particular; the ones that do pass their own.
fn fetcher(base: &str) -> GitHubFetcher {
    GitHubFetcher::at(
        base,
        "ghp_token",
        std::env::temp_dir().join("synapse-fetcher-tests"),
    )
}

fn tarball_returns(body: &[u8]) -> Mock {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/tarball/{SHA}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
}

/// Every file the fetcher has left in its spool directory.
fn spooled(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir).map_or_else(|_| Vec::new(), |d| d.map(|e| e.unwrap().path()).collect())
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
    tarball_returns(b"gzip-bytes").mount(&server).await;
    let spool = tempfile::tempdir().unwrap();

    let fetched = GitHubFetcher::at(server.uri(), "ghp_token", spool.path())
        .fetch(REPO, "main", Some("an-older-sha"))
        .await
        .unwrap();

    match fetched {
        Fetched::Archive { sha, archive } => {
            assert_eq!(sha, SHA, "the trailing newline is trimmed");
            assert_eq!(std::fs::read(archive.path()).unwrap(), b"gzip-bytes");
        }
        Fetched::Unchanged => panic!("expected an archive"),
    }
}

/// The archive is written to disk, never held: a book with its images is hundreds of MB and the
/// container has 256 MiB. The file is the fetch's to clean up, so dropping it must delete it.
#[tokio::test]
async fn the_archive_is_spooled_to_disk_and_deleted_once_dropped() {
    let server = MockServer::start().await;
    head_returns(&server, SHA).await;
    tarball_returns(&vec![7u8; 64 * 1024]).mount(&server).await;
    let spool = tempfile::tempdir().unwrap();

    let fetched = GitHubFetcher::at(server.uri(), "ghp_token", spool.path())
        .fetch(REPO, "main", None)
        .await
        .unwrap();

    let Fetched::Archive { archive, .. } = fetched else {
        panic!("expected an archive")
    };
    assert!(
        archive.path().starts_with(spool.path()),
        "{}",
        archive.path().display()
    );
    assert_eq!(std::fs::metadata(archive.path()).unwrap().len(), 64 * 1024);
    drop(archive);
    assert_eq!(spooled(spool.path()), Vec::<std::path::PathBuf>::new());
}

/// Over the cap is refused, and the partial download does not stay behind on the volume.
#[tokio::test]
async fn an_archive_over_the_cap_is_refused_and_leaves_no_file() {
    let server = MockServer::start().await;
    head_returns(&server, SHA).await;
    tarball_returns(&[7u8; 4096]).mount(&server).await;
    let spool = tempfile::tempdir().unwrap();

    let error = GitHubFetcher::at(server.uri(), "ghp_token", spool.path())
        .capped_at(1024)
        .fetch(REPO, "main", None)
        .await
        .unwrap_err();

    assert!(matches!(error, FetchError::TooLarge(_)), "{error:?}");
    assert!(error.to_string().contains(REPO), "{error}");
    assert_eq!(spooled(spool.path()), Vec::<std::path::PathBuf>::new());
}

#[tokio::test]
async fn a_first_fetch_has_no_known_sha_and_still_pulls() {
    let server = MockServer::start().await;
    head_returns(&server, SHA).await;
    tarball_returns(b"x").mount(&server).await;

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
