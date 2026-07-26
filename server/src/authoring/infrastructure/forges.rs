//! Which forge an edit goes to, resolved per call from the source registry.
//!
//! Resolved rather than cached on purpose. A forge is cheap to mint — a repo, a branch, a token —
//! and edits are human-triggered and rare, so a registry read per edit costs nothing and removes a
//! whole class of staleness: a repository registered a moment ago is editable immediately, and one
//! just removed stops accepting proposals rather than opening pull requests into the void.
//!
//! The primary checkout is deliberately NOT in the registry — it arrives by git-sync — so its
//! target comes from config and is answered here without a query.

use std::sync::Arc;

use crate::authoring::application::{ForgeTarget, Forges};
use crate::authoring::infrastructure::configured::ConfiguredForge;
use crate::catalog::application::ContentSources;
use crate::catalog::domain::content_tree::PRIMARY_SOURCE_ID;

pub struct ConfiguredForges<S> {
    /// Absent in a single-repository deployment: there are no satellites to look up, so every
    /// edit targets the primary and no store is needed to say so.
    sources: Option<Arc<S>>,
    /// `off` · `dry-run` · `github`, applied uniformly: a deployment that cannot open pull
    /// requests against the monorepo must not quietly manage it against a satellite either.
    mode: String,
    /// One platform token across every content repository.
    token: String,
    site_url: String,
    primary: ForgeTarget,
}

impl<S> ConfiguredForges<S> {
    pub fn new(
        sources: Arc<S>,
        mode: impl Into<String>,
        token: impl Into<String>,
        site_url: impl Into<String>,
        primary_repo: impl Into<String>,
        primary_branch: impl Into<String>,
    ) -> Self {
        let site_url = site_url.into();
        Self {
            sources: Some(sources),
            mode: mode.into(),
            token: token.into(),
            primary: ForgeTarget {
                repo: primary_repo.into(),
                base_branch: primary_branch.into(),
                site_url: site_url.clone(),
            },
            site_url,
        }
    }

    /// One repository, no registry — the pre-satellite shape, and what the integration tests run.
    pub fn single(
        mode: impl Into<String>,
        token: impl Into<String>,
        site_url: impl Into<String>,
        repo: impl Into<String>,
        branch: impl Into<String>,
    ) -> Self {
        let site_url = site_url.into();
        Self {
            sources: None,
            mode: mode.into(),
            token: token.into(),
            primary: ForgeTarget {
                repo: repo.into(),
                base_branch: branch.into(),
                site_url: site_url.clone(),
            },
            site_url,
        }
    }

    fn target_of(&self, repo: &str, branch: &str) -> ForgeTarget {
        ForgeTarget {
            repo: repo.to_owned(),
            base_branch: branch.to_owned(),
            site_url: self.site_url.clone(),
        }
    }
}

impl<S: ContentSources> ConfiguredForges<S> {
    /// The branch a repository's proposals target. Only registered satellites are looked up; the
    /// primary answers from config.
    async fn base_branch_of(&self, repo: &str) -> Option<String> {
        if repo == self.primary.repo {
            return Some(self.primary.base_branch.clone());
        }
        match self.sources.as_ref()?.list().await {
            Ok(rows) => rows
                .into_iter()
                .find(|row| row.repo == repo)
                .map(|row| row.branch),
            Err(error) => {
                tracing::warn!(repo, %error, "forge lookup: the source registry is unavailable");
                None
            }
        }
    }
}

impl<S: ContentSources> Forges for ConfiguredForges<S> {
    type Forge = ConfiguredForge;

    async fn target_for(&self, source_id: &str) -> Option<ForgeTarget> {
        if source_id == PRIMARY_SOURCE_ID {
            return Some(self.primary.clone());
        }
        match self.sources.as_ref()?.list().await {
            Ok(rows) => rows
                .into_iter()
                .find(|row| row.id == source_id)
                .map(|row| self.target_of(&row.repo, &row.branch)),
            Err(error) => {
                tracing::warn!(source_id, %error, "forge lookup: the source registry is unavailable");
                None
            }
        }
    }

    async fn forge_for(&self, repo: &str) -> Option<Self::Forge> {
        let branch = self.base_branch_of(repo).await?;
        Some(ConfiguredForge::select(&self.mode, repo, &branch, &self.token))
    }
}
