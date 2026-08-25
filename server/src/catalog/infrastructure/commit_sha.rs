//! The prod content version: the checkout's commit SHA, re-read per call with three tiny file
//! reads and NO `git` binary — the git-sync sidecar advances the SHA with no redeploy, and the
//! version-gated cache rebuilds when it moves. Anything unreadable degrades to `"static"`, never
//! an error.
//!
//! Two checkout shapes reach this, and both must answer: the git-sync'd primary, which has real
//! git metadata, and a fetched satellite, which is an unpacked tarball with none at all. The
//! satellite's commit id is still on disk — the cache names each commit directory after its SHA
//! and flips `current` onto it — so resolving the link is what makes its version move.

use std::fs;
use std::path::{Path, PathBuf};

const FALLBACK: &str = "static";

/// Resolve the checkout's commit SHA, or `"static"` when `content_root` is not a readable
/// checkout (SHA-1 or SHA-256, validated).
pub fn read_commit_sha(content_root: &Path) -> String {
    resolve(content_root)
        .filter(|sha| sha_like(sha))
        .unwrap_or_else(|| FALLBACK.to_owned())
}

fn resolve(content_root: &Path) -> Option<String> {
    match git_dir(content_root) {
        Some(git_dir) => head_sha(&git_dir),
        None => unpacked_commit(content_root),
    }
}

fn head_sha(git_dir: &Path) -> Option<String> {
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref: ") {
        Some(ref_name) => ref_sha(git_dir, ref_name.trim()),
        None => Some(head.to_owned()),
    }
}

/// A fetched satellite has no git metadata: `ContentCache` unpacks each archive into
/// `<cache>/<id>/<sha>/` and flips the `current` symlink onto it, so the commit that landed is the
/// directory the link resolves to.
///
/// Without this a satellite's version is the `"static"` fallback permanently — identical before and
/// after a fetch — so the version-gated catalog cache never rebuilds and newly landed content stays
/// invisible until the process restarts. `sha_like` still guards the result, so a checkout whose
/// directory is not named after a commit degrades to the fallback exactly as before.
fn unpacked_commit(content_root: &Path) -> Option<String> {
    let resolved = fs::canonicalize(content_root).ok()?;
    resolved.file_name()?.to_str().map(str::to_owned)
}

/// `.git` as a directory (plain clone) or a `gitdir: <path>` pointer file (git-sync/worktree).
fn git_dir(content_root: &Path) -> Option<PathBuf> {
    let dot_git = content_root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let pointer = fs::read_to_string(&dot_git).ok()?;
    let target = pointer.trim().strip_prefix("gitdir:")?.trim();
    let path = Path::new(target);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        content_root.join(path)
    })
}

/// A loose ref file, else the `packed-refs` line ending in ` <ref>`.
fn ref_sha(git_dir: &Path, ref_name: &str) -> Option<String> {
    if let Ok(loose) = fs::read_to_string(git_dir.join(ref_name)) {
        return Some(loose.trim().to_owned());
    }
    let packed = fs::read_to_string(git_dir.join("packed-refs")).ok()?;
    packed
        .lines()
        .find(|line| line.ends_with(&format!(" {ref_name}")))
        .and_then(|line| line.split_whitespace().next())
        .map(str::to_owned)
}

fn sha_like(s: &str) -> bool {
    (40..=64).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[cfg(test)]
mod tests;
