//! The prod content version: the checkout's git HEAD SHA, re-read per call with three tiny file
//! reads and NO `git` binary — the git-sync sidecar advances the SHA with no redeploy, and the
//! version-gated cache rebuilds when it moves. Anything unreadable degrades to `"static"`, never
//! an error.

use std::fs;
use std::path::{Path, PathBuf};

const FALLBACK: &str = "static";

/// Resolve the checkout's HEAD SHA, or `"static"` when `content_root` is not a readable
/// checkout (SHA-1 or SHA-256, validated).
pub fn read_commit_sha(content_root: &Path) -> String {
    resolve(content_root)
        .filter(|sha| sha_like(sha))
        .unwrap_or_else(|| FALLBACK.to_owned())
}

fn resolve(content_root: &Path) -> Option<String> {
    let git_dir = git_dir(content_root)?;
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref: ") {
        Some(ref_name) => ref_sha(&git_dir, ref_name.trim()),
        None => Some(head.to_owned()),
    }
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
