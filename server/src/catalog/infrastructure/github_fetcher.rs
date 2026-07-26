//! The GitHub `ContentFetcher`: head check, then archive.
//!
//! Two calls, in that order, because the head check is cheap and almost always the whole story —
//! at a 60-second cadence the overwhelmingly common answer is "nothing moved". Asking for the
//! `sha` media type makes it cheaper still: GitHub answers with the bare commit id rather than a
//! full commit object.
//!
//! Shares the forge's request conventions (bearer, API version, user agent) deliberately: one
//! token, one set of headers, one place where GitHub's error shapes are interpreted.

use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, Response, StatusCode};

use crate::catalog::application::{ContentFetcher, FetchError, Fetched};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
const AGENT: &str = "synapse-rs";
/// A prose book is a few MB; anything approaching this is not a guide.
const MAX_ARCHIVE_BYTES: usize = 128 * 1024 * 1024;

pub struct GitHubFetcher {
    client: Client,
    api_base: String,
    /// Optional: public repositories fetch anonymously, but the token lifts the rate limit from
    /// 60/hour to 5000, which at one tick a minute per source is the difference that matters.
    token: String,
}

impl GitHubFetcher {
    pub fn new(token: impl Into<String>) -> Self {
        Self::at(API, token)
    }

    /// The loopback seam the tests drive.
    pub fn at(api_base: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_mins(1))
                .build()
                .unwrap_or_default(),
            api_base: api_base.into(),
            token: token.into(),
        }
    }

    fn request(&self, url: &str, accept: &str) -> reqwest::RequestBuilder {
        let request = self
            .client
            .get(url)
            .header(ACCEPT, accept)
            .header(USER_AGENT, AGENT)
            .header("X-GitHub-Api-Version", API_VERSION);
        if self.token.is_empty() {
            request
        } else {
            request.header(AUTHORIZATION, format!("Bearer {}", self.token))
        }
    }

    async fn head_sha(&self, repo: &str, branch: &str) -> Result<String, FetchError> {
        let url = format!("{}/repos/{repo}/commits/{branch}", self.api_base);
        let response = self
            .request(&url, "application/vnd.github.sha")
            .send()
            .await
            .map_err(|e| FetchError::Transport(e.to_string()))?;
        let response = check(response, &format!("{repo}@{branch}"))?;
        let sha = response
            .text()
            .await
            .map_err(|e| FetchError::Transport(e.to_string()))?
            .trim()
            .to_owned();
        if sha.is_empty() {
            return Err(FetchError::Malformed(format!("{repo}@{branch}: empty head")));
        }
        Ok(sha)
    }

    async fn archive(&self, repo: &str, sha: &str) -> Result<Vec<u8>, FetchError> {
        let url = format!("{}/repos/{repo}/tarball/{sha}", self.api_base);
        let response = self
            .request(&url, "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| FetchError::Transport(e.to_string()))?;
        let mut response = check(response, &format!("{repo}@{sha}"))?;

        // Streamed with a running cap rather than `bytes()`: codeload does not always send a
        // Content-Length, so a declared size cannot be trusted and an unbounded read would let a
        // hostile archive decide how much memory this process uses.
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| FetchError::Transport(e.to_string()))?
        {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > MAX_ARCHIVE_BYTES {
                return Err(FetchError::TooLarge(format!(
                    "{repo}: archive over {} MiB",
                    MAX_ARCHIVE_BYTES / (1024 * 1024)
                )));
            }
        }
        Ok(bytes)
    }
}

impl ContentFetcher for GitHubFetcher {
    async fn fetch(&self, repo: &str, branch: &str, known_sha: Option<&str>) -> Result<Fetched, FetchError> {
        let sha = self.head_sha(repo, branch).await?;
        if known_sha == Some(sha.as_str()) {
            return Ok(Fetched::Unchanged);
        }
        let bytes = self.archive(repo, &sha).await?;
        tracing::info!(repo, branch, sha, bytes = bytes.len(), "content archive fetched");
        Ok(Fetched::Archive { sha, bytes })
    }
}

/// Map GitHub's answer onto the registry's vocabulary. A rate limit carries its reset so the loop
/// can wait rather than hammer; everything else names the repository, because the message is what
/// an admin reads on the source row when a book stops updating.
fn check(response: Response, what: &str) -> Result<Response, FetchError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if matches!(status, StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS)
        && response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            == Some("0")
    {
        let reset = response
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        return Err(FetchError::RateLimited {
            seconds: reset.saturating_sub(now),
        });
    }
    match status {
        StatusCode::NOT_FOUND => Err(FetchError::NotFound(what.to_owned())),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(FetchError::Denied(format!(
            "{what}: {status} — check the token's repository scope"
        ))),
        _ => Err(FetchError::Transport(format!("{what}: {status}"))),
    }
}

#[cfg(test)]
#[path = "github_fetcher_tests.rs"]
mod tests;
