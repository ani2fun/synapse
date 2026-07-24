//! The GitHub `ContentForge` (`GitHubForge`) — pure REST over `reqwest`.
//!
//! NO `git` binary and NO working copy, which is not a stylistic preference: the production image
//! is debian-slim plus one binary and the Node sidecar, its filesystem is not a place to keep a
//! clone, and a pod that restarts mid-push would leave one in an unknown state. Every operation
//! here is a stateless HTTP call whose failure leaves nothing to clean up.
//!
//! The token is a fine-grained PAT with `contents: write` + `pull_requests: write` on the content
//! repository alone. It is never logged, never returned, and never sent anywhere but
//! `api.github.com`.

use base64::Engine;

use crate::authoring::application::{AuthoringError, ContentForge, ForgePrState};
use crate::authoring::domain::PullRequestRef;
use crate::authoring::infrastructure::github_wire::{
    ContentsResponse, CreatePull, CreateRef, ErrorResponse, PullResponse, PutContents, PutContentsResponse,
    RefResponse,
};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
/// GitHub rejects requests without one.
const USER_AGENT: &str = "synapse-rs";

pub struct GitHubForge {
    client: reqwest::Client,
    /// The API origin — `https://api.github.com` in production; a loopback mock in tests. It is a
    /// genuine parameter (GitHub Enterprise serves a different origin too), not a test-only hook.
    api_base: String,
    /// `owner/name`.
    repo: String,
    owner: String,
    base_branch: String,
    token: String,
}

impl GitHubForge {
    pub fn new(repo: &str, base_branch: &str, token: &str) -> Self {
        Self::at(API, repo, base_branch, token)
    }

    /// Construct against an explicit API origin.
    fn at(api_base: &str, repo: &str, base_branch: &str, token: &str) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            client,
            api_base: api_base.trim_end_matches('/').to_owned(),
            repo: repo.to_owned(),
            owner: repo.split('/').next().unwrap_or(repo).to_owned(),
            base_branch: base_branch.to_owned(),
            token: token.to_owned(),
        }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{path}", self.api_base))
            .bearer_auth(&self.token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION)
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        what: &str,
    ) -> Result<reqwest::Response, AuthoringError> {
        request
            .send()
            .await
            .map_err(|error| AuthoringError::ForgeUnavailable(format!("{what}: {error}")))
    }

    /// The head sha of a branch; `None` when the ref does not exist yet.
    async fn head_of(&self, branch: &str) -> Result<Option<String>, AuthoringError> {
        let path = format!("/repos/{}/git/ref/heads/{branch}", self.repo);
        let response = self
            .send(self.request(reqwest::Method::GET, &path), "read branch")
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let body: RefResponse = decode(response, "read branch").await?;
        Ok(Some(body.object.sha))
    }

    /// Create `branch` off the default branch. A concurrent creator's 422 is treated as success —
    /// the ref exists either way, which is all the caller needs.
    async fn create_branch(&self, branch: &str) -> Result<(), AuthoringError> {
        let base = self
            .head_of(&self.base_branch)
            .await?
            .ok_or_else(|| AuthoringError::ForgeUnavailable(format!("no '{}' branch", self.base_branch)))?;
        let path = format!("/repos/{}/git/refs", self.repo);
        let ref_name = format!("refs/heads/{branch}");
        let payload = CreateRef {
            ref_name: &ref_name,
            sha: &base,
        };
        let response = self
            .send(
                self.request(reqwest::Method::POST, &path).json(&payload),
                "create branch",
            )
            .await?;
        if response.status().is_success() || response.status() == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
            tracing::debug!(branch, base = %base, "branch ready");
            return Ok(());
        }
        Err(failed(response, "create branch").await)
    }

    /// The blob sha of a file on a branch; `None` when the file is not there.
    async fn blob_sha(&self, branch: &str, file_path: &str) -> Result<Option<String>, AuthoringError> {
        let path = format!("/repos/{}/contents/{file_path}?ref={branch}", self.repo);
        let response = self
            .send(self.request(reqwest::Method::GET, &path), "read file")
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let body: ContentsResponse = decode(response, "read file").await?;
        Ok(Some(body.sha))
    }

    /// The still-open pull request whose head is `branch`, if any.
    async fn open_pull_for(&self, branch: &str) -> Result<Option<PullRequestRef>, AuthoringError> {
        let path = format!(
            "/repos/{}/pulls?state=open&head={}:{branch}",
            self.repo, self.owner
        );
        let response = self
            .send(self.request(reqwest::Method::GET, &path), "list pull requests")
            .await?;
        let body: Vec<PullResponse> = decode(response, "list pull requests").await?;
        Ok(body.first().map(|pull| PullRequestRef {
            number: pull.number,
            url: pull.html_url.clone(),
        }))
    }
}

impl ContentForge for GitHubForge {
    fn mode(&self) -> &'static str {
        "github"
    }

    async fn commit_file(
        &self,
        branch: &str,
        file_path: &str,
        content: &str,
        message: &str,
    ) -> Result<String, AuthoringError> {
        if self.head_of(branch).await?.is_none() {
            self.create_branch(branch).await?;
        }
        let existing = self.blob_sha(branch, file_path).await?;
        let payload = PutContents {
            message,
            content: base64::engine::general_purpose::STANDARD.encode(content),
            branch,
            sha: existing.as_deref(),
        };
        let path = format!("/repos/{}/contents/{file_path}", self.repo);
        let response = self
            .send(
                self.request(reqwest::Method::PUT, &path).json(&payload),
                "commit file",
            )
            .await?;
        // GitHub's own optimistic-concurrency answer: the blob moved between our read and our
        // write. The contributor is told to reload rather than having their copy win silently.
        if response.status() == reqwest::StatusCode::CONFLICT {
            return Err(AuthoringError::SourceMoved(file_path.to_owned()));
        }
        let body: PutContentsResponse = decode(response, "commit file").await?;
        tracing::info!(branch, file_path, commit = %body.commit.sha, "committed to the content repository");
        Ok(body.commit.sha)
    }

    async fn open_pull_request(
        &self,
        branch: &str,
        title: &str,
        body: &str,
    ) -> Result<Option<PullRequestRef>, AuthoringError> {
        let payload = CreatePull {
            title,
            head: branch,
            base: &self.base_branch,
            body,
        };
        let path = format!("/repos/{}/pulls", self.repo);
        let response = self
            .send(
                self.request(reqwest::Method::POST, &path).json(&payload),
                "open pull request",
            )
            .await?;
        // 422 is how GitHub says "one already exists for this head" — which is exactly the state a
        // retry after a failed store write leaves behind. Returning the existing one is what makes
        // this port's idempotence promise true, so the service can safely commit before it records.
        if response.status() == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
            if let Some(existing) = self.open_pull_for(branch).await? {
                tracing::info!(
                    branch,
                    pr = existing.number,
                    "reusing the pull request already open"
                );
                return Ok(Some(existing));
            }
            return Err(failed(response, "open pull request").await);
        }
        let pull: PullResponse = decode(response, "open pull request").await?;
        tracing::info!(branch, pr = pull.number, "opened a pull request");
        Ok(Some(PullRequestRef {
            number: pull.number,
            url: pull.html_url,
        }))
    }

    async fn pull_request_state(&self, number: u64) -> Result<ForgePrState, AuthoringError> {
        let path = format!("/repos/{}/pulls/{number}", self.repo);
        let response = self
            .send(self.request(reqwest::Method::GET, &path), "read pull request")
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(ForgePrState::Missing);
        }
        let pull: PullResponse = decode(response, "read pull request").await?;
        Ok(if pull.is_merged() {
            ForgePrState::Merged
        } else if pull.state == "open" {
            ForgePrState::Open
        } else {
            ForgePrState::Closed
        })
    }
}

/// A non-2xx becomes a `ForgeUnavailable` carrying GitHub's own message; a 2xx that will not
/// decode is the same kind of failure, since neither leaves us anything to act on.
async fn decode<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    what: &str,
) -> Result<T, AuthoringError> {
    if !response.status().is_success() {
        return Err(failed(response, what).await);
    }
    response
        .json()
        .await
        .map_err(|error| AuthoringError::ForgeUnavailable(format!("{what}: undecodable response: {error}")))
}

/// The status plus GitHub's `message`. A 401/403 is called out by name because "the token expired
/// or lost its scopes" is the failure an operator will actually hit, and a bare 403 reads like a
/// permissions bug in our own code.
async fn failed(response: reqwest::Response, what: &str) -> AuthoringError {
    let status = response.status();
    let message = response
        .json::<ErrorResponse>()
        .await
        .map(|body| body.message)
        .unwrap_or_default();
    let detail = if message.is_empty() {
        format!("{what}: GitHub answered {status}")
    } else {
        format!("{what}: GitHub answered {status} — {message}")
    };
    if matches!(
        status,
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        return AuthoringError::ForgeUnavailable(format!(
            "{detail} (check the token's expiry and its contents/pull-requests scopes)"
        ));
    }
    AuthoringError::ForgeUnavailable(detail)
}

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
