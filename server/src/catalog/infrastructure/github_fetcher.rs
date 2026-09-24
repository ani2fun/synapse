//! The GitHub `ContentFetcher`: head check, then archive.
//!
//! Two calls, in that order, because the head check is cheap and almost always the whole story —
//! at a 60-second cadence the overwhelmingly common answer is "nothing moved". Asking for the
//! `sha` media type makes it cheaper still: GitHub answers with the bare commit id rather than a
//! full commit object.
//!
//! Shares the forge's request conventions (bearer, API version, user agent) deliberately: one
//! token, one set of headers, one place where GitHub's error shapes are interpreted.

use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, Response, StatusCode};
use tokio::io::AsyncWriteExt;

use crate::catalog::application::{ContentFetcher, FetchError, Fetched, SpooledArchive};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
const AGENT: &str = "synapse-rs";
/// A prose book is a few MB, but a book that carries its own images is hundreds: dsa-helmsman is
/// ~700 MB compressed. The archive streams to disk, so this bounds the cache volume, not memory.
const MAX_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;
/// The whole request, body included. A 1 GiB archive at a modest 10 MB/s takes ~100 s.
const ARCHIVE_TIMEOUT: Duration = Duration::from_mins(10);

pub struct GitHubFetcher {
    client: Client,
    api_base: String,
    /// Optional: public repositories fetch anonymously, but the token lifts the rate limit from
    /// 60/hour to 5000, which at one tick a minute per source is the difference that matters.
    token: String,
    /// Where archives are written while they download. The content cache's own volume, so an
    /// archive costs disk the cache already budgets for; never memory.
    spool: PathBuf,
    max_archive_bytes: u64,
}

impl GitHubFetcher {
    pub fn new(token: impl Into<String>, spool: impl Into<PathBuf>) -> Self {
        Self::at(API, token, spool)
    }

    /// The loopback seam the tests drive.
    pub fn at(api_base: impl Into<String>, token: impl Into<String>, spool: impl Into<PathBuf>) -> Self {
        Self {
            client: Client::builder()
                .timeout(ARCHIVE_TIMEOUT)
                .build()
                .unwrap_or_default(),
            api_base: api_base.into(),
            token: token.into(),
            spool: spool.into(),
            max_archive_bytes: MAX_ARCHIVE_BYTES,
        }
    }

    /// A lower cap, so a test can prove the refusal without a gigabyte of body.
    #[cfg(test)]
    fn capped_at(mut self, bytes: u64) -> Self {
        self.max_archive_bytes = bytes;
        self
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

    async fn archive(&self, repo: &str, sha: &str) -> Result<SpooledArchive, FetchError> {
        let url = format!("{}/repos/{repo}/tarball/{sha}", self.api_base);
        let response = self
            .request(&url, "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| FetchError::Transport(e.to_string()))?;
        let mut response = check(response, &format!("{repo}@{sha}"))?;

        // Streamed to disk with a running cap rather than `bytes()`: codeload does not always
        // send a Content-Length, so a declared size cannot be trusted, and holding the body would
        // let the repository decide how much memory this process uses. Adopted before the first
        // write, so every early return below deletes the partial file.
        let archive = SpooledArchive::adopt(spool_path(&self.spool, repo, sha));
        let mut file = spool_file(archive.path()).await?;
        let mut written: u64 = 0;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| FetchError::Transport(e.to_string()))?
        {
            written += chunk.len() as u64;
            if written > self.max_archive_bytes {
                return Err(FetchError::TooLarge(format!(
                    "{repo}: archive over {} MiB",
                    self.max_archive_bytes / (1024 * 1024)
                )));
            }
            file.write_all(&chunk).await.map_err(|e| spool_failed(&e))?;
        }
        file.flush().await.map_err(|e| spool_failed(&e))?;
        Ok(archive)
    }
}

impl ContentFetcher for GitHubFetcher {
    async fn fetch(&self, repo: &str, branch: &str, known_sha: Option<&str>) -> Result<Fetched, FetchError> {
        let sha = self.head_sha(repo, branch).await?;
        if known_sha == Some(sha.as_str()) {
            return Ok(Fetched::Unchanged);
        }
        let archive = self.archive(repo, &sha).await?;
        let bytes = std::fs::metadata(archive.path()).map_or(0, |m| m.len());
        tracing::info!(repo, branch, sha, bytes, "content archive fetched");
        Ok(Fetched::Archive { sha, archive })
    }
}

/// A dot-file in the cache root. The cache lists its sources by DIRECTORY, so a spool file is
/// never mistaken for one — and never reclaimed from under a download.
fn spool_path(spool: &Path, repo: &str, sha: &str) -> PathBuf {
    spool.join(format!(".fetch-{}-{sha}.tar.gz", repo.replace('/', "_")))
}

async fn spool_file(path: &Path) -> Result<tokio::fs::File, FetchError> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| spool_failed(&e))?;
    }
    tokio::fs::File::create(path).await.map_err(|e| spool_failed(&e))
}

fn spool_failed(error: &std::io::Error) -> FetchError {
    FetchError::Transport(format!("spooling the archive to disk: {error}"))
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
mod tests;
