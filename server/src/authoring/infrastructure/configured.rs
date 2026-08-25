//! The forge the deployment is configured for (`ConfiguredForge`).
//!
//! A small enum rather than a trait object, because `ContentForge` returns `impl Future` and is
//! not dyn-compatible — and rather than a generic threaded through the routes and the wiring
//! struct, because exactly two variants exist and neither is a test seam: the dry run is a
//! shipped mode. One concrete type keeps `ProposeEdit`'s production spelling to a single alias.

use crate::authoring::application::{AuthoringError, ContentForge, ForgePrState};
use crate::authoring::domain::PullRequestRef;
use crate::authoring::infrastructure::{DryRunForge, GitHubForge};

pub enum ConfiguredForge {
    GitHub(GitHubForge),
    DryRun(DryRunForge),
}

impl ConfiguredForge {
    /// `github` needs a token; asking for it without one is a MISCONFIGURATION, and falling back
    /// to a dry run would mean a production deployment silently accepting edits it never
    /// forwards. So it degrades loudly: the warning names the missing variable, and the mode the
    /// client is told is the dry run it actually got.
    pub fn select(mode: &str, repo: &str, base_branch: &str, token: &str) -> Self {
        if mode == "github" && !token.trim().is_empty() {
            tracing::info!(repo, base_branch, "content forge: GitHub pull requests");
            return Self::GitHub(GitHubForge::new(repo, base_branch, token));
        }
        if mode == "github" {
            tracing::warn!(
                repo,
                "content forge: GITHUB_TOKEN is empty — falling back to a DRY RUN; edits will be \
                 validated and recorded but no pull request will be opened"
            );
        } else {
            tracing::info!(repo, base_branch, "content forge: dry run (no pull requests)");
        }
        Self::DryRun(DryRunForge::new(repo, base_branch))
    }
}

impl ContentForge for ConfiguredForge {
    fn mode(&self) -> &'static str {
        match self {
            Self::GitHub(forge) => forge.mode(),
            Self::DryRun(forge) => forge.mode(),
        }
    }

    async fn commit_file(
        &self,
        branch: &str,
        file_path: &str,
        content: &str,
        message: &str,
    ) -> Result<String, AuthoringError> {
        match self {
            Self::GitHub(forge) => forge.commit_file(branch, file_path, content, message).await,
            Self::DryRun(forge) => forge.commit_file(branch, file_path, content, message).await,
        }
    }

    async fn open_pull_request(
        &self,
        branch: &str,
        title: &str,
        body: &str,
    ) -> Result<Option<PullRequestRef>, AuthoringError> {
        match self {
            Self::GitHub(forge) => forge.open_pull_request(branch, title, body).await,
            Self::DryRun(forge) => forge.open_pull_request(branch, title, body).await,
        }
    }

    async fn pull_request_state(&self, number: u64) -> Result<ForgePrState, AuthoringError> {
        match self {
            Self::GitHub(forge) => forge.pull_request_state(number).await,
            Self::DryRun(forge) => forge.pull_request_state(number).await,
        }
    }
}

#[cfg(test)]
#[path = "configured_tests.rs"]
mod tests;
