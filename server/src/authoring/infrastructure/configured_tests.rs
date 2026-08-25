//! Which forge a configuration selects. The case that must not surprise anyone is `github`
//! without a token: it DEGRADES to a dry run that says so, rather than failing at the first PR.

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn a_token_selects_github() {
    let forge = ConfiguredForge::select("github", "a/b", "main", "ghp_x");
    assert_eq!(forge.mode(), "github");
}

#[test]
fn github_without_a_token_degrades_to_a_dry_run_that_says_so() {
    // The mode the client is told must be the one that actually ran, or a contributor is
    // shown "submitted" for something that never left the process.
    let forge = ConfiguredForge::select("github", "a/b", "main", "  ");
    assert_eq!(forge.mode(), "dry-run");
}

#[test]
fn anything_else_is_a_dry_run() {
    for mode in ["dry-run", "", "typo"] {
        assert_eq!(
            ConfiguredForge::select(mode, "a/b", "main", "ghp_x").mode(),
            "dry-run"
        );
    }
}
