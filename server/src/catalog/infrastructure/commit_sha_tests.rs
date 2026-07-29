//! Both checkout shapes must produce a version that MOVES when their content does — that is the
//! whole job. The git-sync'd primary moves because its HEAD advances; a fetched satellite moves
//! because `current` is flipped onto a new commit directory. A satellite that answered `"static"`
//! forever is what let landed content stay invisible until a restart.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;

use super::*;

const SHA_A: &str = "d2dad749889b3dd4471356da8e47fba9fae42e21";
const SHA_B: &str = "4ecf01b5a5acc83dd2b64ebd2c9f054de868aa53";

/// A checkout with real git metadata: `.git/HEAD` pointing at a loose ref.
fn git_checkout(root: &Path, sha: &str) {
    let git = root.join(".git");
    fs::create_dir_all(git.join("refs/heads")).unwrap();
    fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(git.join("refs/heads/main"), format!("{sha}\n")).unwrap();
}

/// The cache's layout: `<root>/<sha>/` with `current` symlinked onto it.
fn unpacked_checkout(source_dir: &Path, sha: &str) -> PathBuf {
    let commit_dir = source_dir.join(sha);
    fs::create_dir_all(&commit_dir).unwrap();
    let link = source_dir.join("current");
    let _ = fs::remove_file(&link);
    #[cfg(unix)]
    std::os::unix::fs::symlink(&commit_dir, &link).unwrap();
    #[cfg(not(unix))]
    std::os::windows::fs::symlink_dir(&commit_dir, &link).unwrap();
    link
}

#[test]
fn a_git_checkout_resolves_its_head_through_a_loose_ref() {
    let tmp = tempfile::tempdir().unwrap();
    git_checkout(tmp.path(), SHA_A);

    assert_eq!(read_commit_sha(tmp.path()), SHA_A);
}

#[test]
fn a_detached_head_is_the_sha_itself() {
    let tmp = tempfile::tempdir().unwrap();
    let git = tmp.path().join(".git");
    fs::create_dir_all(&git).unwrap();
    fs::write(git.join("HEAD"), format!("{SHA_B}\n")).unwrap();

    assert_eq!(read_commit_sha(tmp.path()), SHA_B);
}

#[test]
fn an_unpacked_satellite_resolves_the_commit_its_current_link_points_at() {
    let tmp = tempfile::tempdir().unwrap();
    let checkout = unpacked_checkout(tmp.path(), SHA_A);

    assert_eq!(read_commit_sha(&checkout), SHA_A);
}

/// The regression this file exists for: the satellite's version must CHANGE when a new commit is
/// published, or the version-gated catalog cache serves the old index forever.
#[test]
fn flipping_current_onto_a_new_commit_moves_the_version() {
    let tmp = tempfile::tempdir().unwrap();

    let checkout = unpacked_checkout(tmp.path(), SHA_A);
    let before = read_commit_sha(&checkout);

    let checkout = unpacked_checkout(tmp.path(), SHA_B);
    let after = read_commit_sha(&checkout);

    assert_eq!(before, SHA_A);
    assert_eq!(after, SHA_B);
    assert_ne!(before, after, "a landed commit must move the content version");
}

#[test]
fn a_checkout_that_is_neither_degrades_to_the_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let plain = tmp.path().join("synapse-content");
    fs::create_dir_all(&plain).unwrap();

    assert_eq!(read_commit_sha(&plain), FALLBACK);
}

#[test]
fn a_missing_root_degrades_to_the_fallback() {
    let tmp = tempfile::tempdir().unwrap();

    assert_eq!(read_commit_sha(&tmp.path().join("never-landed")), FALLBACK);
}

/// Guards the `sha_like` filter through the new path: a commit directory is named after a SHA, so
/// anything else is not a commit id and must not become one.
#[test]
fn a_directory_not_named_after_a_commit_is_not_a_version() {
    let tmp = tempfile::tempdir().unwrap();
    let checkout = unpacked_checkout(tmp.path(), "not-a-sha");

    assert_eq!(read_commit_sha(&checkout), FALLBACK);
}
