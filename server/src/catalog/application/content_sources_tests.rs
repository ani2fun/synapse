//! Registration validation. One theme: reject at the door what the catalog cannot serve — the
//! grouping in particular, because nothing downstream slug-checks it before it reaches the sitemap.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn draft(repo: &str, grouping: &str) -> ContentSourceDraft {
    ContentSourceDraft {
        repo: repo.to_owned(),
        branch: "main".to_owned(),
        grouping: grouping_from_str(grouping),
        order: None,
        enabled: true,
    }
}

#[test]
fn the_id_comes_from_the_repository_name() {
    assert_eq!(ContentSourceDraft::derive_id("ani2fun/java-guide"), "java-guide");
    assert_eq!(ContentSourceDraft::derive_id("ani2fun/DSA_Guide"), "dsa_guide");
}

#[test]
fn a_well_formed_registration_yields_its_id() {
    let ok = draft("ani2fun/java-guide", "programming-languages").validate();
    assert_eq!(ok.unwrap(), "java-guide");
}

#[test]
fn the_top_level_is_an_empty_grouping() {
    assert!(grouping_from_str("").is_empty());
    assert!(grouping_from_str("  /  ").is_empty());
    assert_eq!(grouping_from_str("a/b"), vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn a_grouping_segment_that_is_not_slug_like_is_refused() {
    // Unchecked, this reaches <loc> in the sitemap by way of the book's category path.
    let error = draft("ani2fun/x-guide", "not a slug").validate().unwrap_err();
    assert!(matches!(error, RegistryError::Invalid(_)), "{error:?}");
}

#[test]
fn a_repo_that_is_not_owner_slash_name_is_refused() {
    for repo in ["java-guide", "ani2fun/", "/java-guide", "a/b/c"] {
        assert!(
            draft(repo, "").validate().is_err(),
            "expected '{repo}' to be refused"
        );
    }
}

#[test]
fn a_blank_branch_is_refused() {
    let mut d = draft("ani2fun/java-guide", "");
    d.branch = "  ".to_owned();
    assert!(d.validate().is_err());
}

#[test]
fn a_records_placement_is_what_the_merge_grafts_by() {
    let record = ContentSourceRecord {
        id: "java-guide".to_owned(),
        repo: "ani2fun/java-guide".to_owned(),
        branch: "main".to_owned(),
        grouping: vec!["programming-languages".to_owned()],
        order: Some(7),
        enabled: true,
        last_sha: None,
        last_synced_at: None,
        last_error: None,
    };
    let placement = record.placement();
    assert_eq!(placement.source_id, "java-guide");
    assert_eq!(placement.grouping, vec!["programming-languages".to_owned()]);
    assert_eq!(placement.order, Some(7));
}
