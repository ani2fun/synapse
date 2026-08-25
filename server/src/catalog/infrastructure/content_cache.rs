//! Where fetched satellites live on disk, and the only code that unpacks an archive.
//!
//! **This is untrusted-archive handling.** The bytes come from a forge, and a tar entry can name
//! `../../etc/whatever`, an absolute path, or a symlink pointing anywhere it likes. Every guard
//! therefore lives in the unpacker rather than in a later path check: by the time a caller could
//! validate a path, the write has already happened.
//!
//! The layout mirrors git-sync's, because its atomicity trick is the right one:
//!
//! ```text
//! <cache>/<source id>/<sha>/…     one directory per commit, written in full before it is used
//! <cache>/<source id>/current  →  <sha>     flipped only once the unpack succeeded
//! ```
//!
//! A reader therefore never sees a half-written tree: it either follows `current` to a complete
//! checkout or to the previous one. The flip is a symlink rename, and older commits are pruned
//! after it, never before.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;

use crate::catalog::application::FetchError;

/// Refusal thresholds. Generous for prose — the largest book in the corpus is a few MB — and far
/// below anything that could fill the cache volume.
const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;

/// The `current` symlink name, matching git-sync's so the two layouts read alike.
const CURRENT: &str = "current";

pub struct ContentCache {
    root: PathBuf,
}

impl ContentCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Where a source's live checkout is, whether or not anything has landed there yet. Handing
    /// this out before the first fetch is deliberate: the repository treats a missing root as an
    /// empty source, so a satellite that has not synced yet is an absent book, not a broken boot.
    #[must_use]
    pub fn checkout_of(&self, source_id: &str) -> PathBuf {
        self.root.join(source_id).join(CURRENT)
    }

    /// Unpack an archive into its own commit directory, then flip `current` onto it.
    pub fn publish(&self, source_id: &str, sha: &str, archive: &[u8]) -> Result<PathBuf, FetchError> {
        let source_dir = self.root.join(source_id);
        let commit_dir = source_dir.join(sha);
        // A leftover from a crashed unpack must not be merged with this one.
        if commit_dir.exists() {
            fs::remove_dir_all(&commit_dir).map_err(|e| io_failed(&e))?;
        }
        fs::create_dir_all(&commit_dir).map_err(|e| io_failed(&e))?;

        if let Err(error) = unpack(archive, &commit_dir) {
            // Never leave a partial tree where `current` might later be pointed at it.
            let _ = fs::remove_dir_all(&commit_dir);
            return Err(error);
        }

        let link = source_dir.join(CURRENT);
        replace_symlink(&commit_dir, &link)?;
        prune_other_commits(&source_dir, sha);
        Ok(link)
    }

    /// Drop a source's cache entirely — used when its registration goes away.
    pub fn forget(&self, source_id: &str) {
        let _ = fs::remove_dir_all(self.root.join(source_id));
    }

    /// Every source with a directory here, registered or not. The sync loop diffs this against
    /// the registry to reclaim what nobody claims any more.
    #[must_use]
    pub fn cached_source_ids(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect()
    }
}

fn io_failed(error: &std::io::Error) -> FetchError {
    FetchError::Malformed(error.to_string())
}

/// Replace atomically where the platform allows it: write the new link beside the old one and
/// rename over it, so a reader following `current` never finds it briefly absent.
fn replace_symlink(target: &Path, link: &Path) -> Result<(), FetchError> {
    let staging = link.with_extension("next");
    let _ = fs::remove_file(&staging);
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, &staging).map_err(|e| io_failed(&e))?;
    #[cfg(not(unix))]
    std::os::windows::fs::symlink_dir(target, &staging).map_err(|e| io_failed(&e))?;
    fs::rename(&staging, link).map_err(|e| io_failed(&e))
}

/// Every commit directory except the live one. Pruning AFTER the flip means a failure here leaves
/// disk to reclaim, never a dangling `current`.
fn prune_other_commits(source_dir: &Path, keep: &str) {
    let Ok(entries) = fs::read_dir(source_dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == keep || name == CURRENT {
            continue;
        }
        if entry.path().is_dir() {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Gunzip + untar into `dest`, refusing anything that would escape it.
///
/// GitHub roots its tarballs at `{owner}-{repo}-{sha7}/`, so one leading component is stripped —
/// which is also why the guard cannot simply trust `Archive::unpack`'s own checks: the paths are
/// rewritten here, and a rewritten path must be re-validated.
fn unpack(archive: &[u8], dest: &Path) -> Result<(), FetchError> {
    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    // Symlinks and hard links are the sharpest edge in an untrusted archive: a link may point
    // outside `dest`, and a later regular entry written "through" it escapes. Prose needs
    // neither, so they are dropped rather than validated.
    tar.set_unpack_xattrs(false);
    tar.set_preserve_permissions(false);

    let entries = tar.entries().map_err(|e| FetchError::Malformed(e.to_string()))?;
    let mut written_bytes: u64 = 0;
    let mut written_entries: usize = 0;

    for entry in entries {
        let mut entry = entry.map_err(|e| FetchError::Malformed(e.to_string()))?;
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| FetchError::Malformed(e.to_string()))?
            .into_owned();
        // Refused rather than relativised. Stripping the wrapping component would turn
        // `/etc/passwd` into a harmless `etc/passwd` inside the commit directory, but quietly
        // rewriting a hostile name is how a guard stops being one — a forge archive has no
        // business naming an absolute path at all.
        if path.is_absolute() {
            return Err(FetchError::Malformed(format!(
                "archive entry is an absolute path: {}",
                path.display()
            )));
        }
        let Some(relative) = strip_root(&path) else {
            continue;
        };
        let Some(target) = safe_join(dest, &relative) else {
            return Err(FetchError::Malformed(format!(
                "archive entry escapes its root: {}",
                path.display()
            )));
        };

        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|e| io_failed(&e))?;
            continue;
        }
        if !kind.is_file() {
            continue;
        }

        written_entries += 1;
        if written_entries > MAX_ENTRIES {
            return Err(FetchError::TooLarge(format!("over {MAX_ENTRIES} entries")));
        }
        written_bytes = written_bytes.saturating_add(entry.header().size().unwrap_or(0));
        if written_bytes > MAX_UNPACKED_BYTES {
            return Err(FetchError::TooLarge(format!(
                "unpacks to over {} MiB",
                MAX_UNPACKED_BYTES / (1024 * 1024)
            )));
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| io_failed(&e))?;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| io_failed(&e))?;
        fs::write(&target, &bytes).map_err(|e| io_failed(&e))?;
    }
    Ok(())
}

/// Drop the archive's single wrapping directory. An entry that IS that directory yields `None`.
fn strip_root(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    components.next()?;
    let rest: PathBuf = components.collect();
    (!rest.as_os_str().is_empty()).then_some(rest)
}

/// Join only what stays inside `dest`: no `..`, no absolute path, no root or prefix component.
/// Purely lexical on purpose — it must hold BEFORE anything is created, so it cannot rely on
/// canonicalising a path that does not exist yet.
fn safe_join(dest: &Path, relative: &Path) -> Option<PathBuf> {
    let mut out = dest.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => out.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests;
