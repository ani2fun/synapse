//! The dry-run `ContentForge` — what a deployment with no GitHub token gets.
//!
//! It is not a mock and not a stub: it is a real, shipped mode. Dev, CI and the e2e stack all run
//! the whole editing flow against it — the allowlist gate, the drift guard, the validation, the
//! branch derivation, the reuse rule and the stored history are all exercised for real, and only
//! the network call at the very end is skipped.
//!
//! The one thing it must never do is let a contributor believe their change was proposed. It opens
//! no pull request and reports `None`, which is what makes `mode` show up in the API answer and
//! the editor say plainly that nothing was submitted.

use crate::authoring::application::{AuthoringError, ContentForge, ForgePrState};
use crate::authoring::domain::PullRequestRef;

pub struct DryRunForge {
    repo: String,
    base_branch: String,
}

impl DryRunForge {
    pub fn new(repo: &str, base_branch: &str) -> Self {
        Self {
            repo: repo.to_owned(),
            base_branch: base_branch.to_owned(),
        }
    }
}

impl ContentForge for DryRunForge {
    fn mode(&self) -> &'static str {
        "dry-run"
    }

    async fn commit_file(
        &self,
        branch: &str,
        file_path: &str,
        content: &str,
        message: &str,
    ) -> Result<String, AuthoringError> {
        let subject = message.lines().next().unwrap_or_default();
        tracing::info!(
            repo = self.repo,
            base = self.base_branch,
            branch,
            file_path,
            bytes = content.len(),
            subject,
            "dry run — this WOULD have been committed to the content repository"
        );
        Ok("dry-run".to_owned())
    }

    /// Nothing is opened, so there is nothing to point at. The stored request keeps its branch and
    /// its history; only the pull request is absent.
    async fn open_pull_request(
        &self,
        branch: &str,
        title: &str,
        _body: &str,
    ) -> Result<Option<PullRequestRef>, AuthoringError> {
        tracing::info!(
            repo = self.repo,
            branch,
            title,
            "dry run — no pull request opened (this deployment has no forge token)"
        );
        Ok(None)
    }

    /// Never reached in practice: a dry-run request carries no pull request, so the service's
    /// reuse probe short-circuits before asking. Answering `Missing` keeps the honest reading —
    /// there is no pull request here.
    async fn pull_request_state(&self, _number: u64) -> Result<ForgePrState, AuthoringError> {
        Ok(ForgePrState::Missing)
    }
}

#[cfg(test)]
mod tests;
