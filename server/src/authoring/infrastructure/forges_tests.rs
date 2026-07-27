//! Which forge an edit reaches. The interesting cases are all about a repository the registry
//! knows nothing about — a satellite just removed, a store that is down — because those decide
//! whether a proposal is refused or opened somewhere wrong.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use super::*;
use crate::catalog::application::{
    ContentSourceDraft, ContentSourceRecord, RegistryError, SyncOutcome, grouping_from_str,
};

struct FakeRegistry {
    rows: Vec<ContentSourceRecord>,
    fails: bool,
}

impl FakeRegistry {
    fn with(rows: Vec<ContentSourceRecord>) -> Arc<Self> {
        Arc::new(Self { rows, fails: false })
    }
    fn unavailable() -> Arc<Self> {
        Arc::new(Self {
            rows: Vec::new(),
            fails: true,
        })
    }
}

impl ContentSources for FakeRegistry {
    async fn list(&self) -> Result<Vec<ContentSourceRecord>, RegistryError> {
        if self.fails {
            return Err(RegistryError::StoreFailed("down".to_owned()));
        }
        Ok(self.rows.clone())
    }
    async fn upsert(&self, _: &ContentSourceDraft) -> Result<ContentSourceRecord, RegistryError> {
        unimplemented!()
    }
    async fn remove(&self, _: &str) -> Result<bool, RegistryError> {
        unimplemented!()
    }
    async fn record_sync(&self, _: &str, _: &SyncOutcome) -> Result<(), RegistryError> {
        Ok(())
    }
}

fn satellite() -> ContentSourceRecord {
    ContentSourceRecord {
        id: "java-guide".to_owned(),
        repo: "ani2fun/java-guide".to_owned(),
        branch: "trunk".to_owned(),
        grouping: grouping_from_str("programming-languages"),
        order: None,
        enabled: true,
        last_sha: None,
        last_synced_at: None,
        last_error: None,
    }
}

fn forges(registry: Arc<FakeRegistry>) -> ConfiguredForges<FakeRegistry> {
    ConfiguredForges::new(
        registry,
        "dry-run",
        "",
        "https://synapse.test",
        "ani2fun/synapse-content",
        "main",
    )
}

#[tokio::test]
async fn the_primary_answers_from_config_without_touching_the_registry() {
    // Even with the store down: the spine is not a row, so it must not depend on one.
    let target = forges(FakeRegistry::unavailable())
        .target_for(PRIMARY_SOURCE_ID)
        .await
        .expect("the primary always has a target");
    assert_eq!(target.repo, "ani2fun/synapse-content");
    assert_eq!(target.base_branch, "main");
    assert_eq!(target.site_url, "https://synapse.test");
}

#[tokio::test]
async fn a_satellite_target_comes_from_its_registration() {
    let target = forges(FakeRegistry::with(vec![satellite()]))
        .target_for("java-guide")
        .await
        .expect("a registered source has a target");
    assert_eq!(target.repo, "ani2fun/java-guide");
    assert_eq!(target.base_branch, "trunk", "the row's branch, not the primary's");
}

#[tokio::test]
async fn an_unregistered_source_has_no_target() {
    assert!(
        forges(FakeRegistry::with(Vec::new()))
            .target_for("gone-guide")
            .await
            .is_none()
    );
}

/// A store that is down must not silently route a satellite's edit at the primary — that would
/// open a pull request against the wrong repository.
#[tokio::test]
async fn an_unavailable_registry_refuses_rather_than_falling_back() {
    let forges = forges(FakeRegistry::unavailable());
    assert!(forges.target_for("java-guide").await.is_none());
    assert!(forges.forge_for("ani2fun/java-guide").await.is_none());
}

#[tokio::test]
async fn a_forge_is_minted_for_a_known_repository_only() {
    let forges = forges(FakeRegistry::with(vec![satellite()]));
    assert!(forges.forge_for("ani2fun/synapse-content").await.is_some());
    assert!(forges.forge_for("ani2fun/java-guide").await.is_some());
    assert!(
        forges.forge_for("someone/else").await.is_none(),
        "an unknown repository has no branch, so no forge"
    );
}

/// The single-repository shape: no store at all, every source answering with the primary.
#[tokio::test]
async fn the_registry_free_form_needs_no_store() {
    let forges = ConfiguredForges::<FakeRegistry>::single(
        "dry-run",
        "",
        "https://synapse.test",
        "test/content",
        "main",
    );
    let target = forges.target_for("anything").await.unwrap();
    assert_eq!(target.repo, "test/content");
    assert!(forges.forge_for("test/content").await.is_some());
}
