//! The filesystem `ContentRepository` (oracle: `FileSystemContentRepositoryLive`) — walks
//! `SYNAPSE_ROOT`, decodes `book.json`/`category.json` leniently at every level, guards lesson
//! reads against traversal (realpaths BOTH sides — macOS `/tmp` is a symlink), and produces the
//! ADR-S010 content version (dev watermark / prod git SHA).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::de::DeserializeOwned;

use crate::catalog::application::{ContentError, ContentRepository};
use crate::catalog::domain::content_tree::{BookMeta, CategoryMeta, ContentEntry};
use crate::catalog::infrastructure::commit_sha::read_commit_sha;
use crate::platform::blocking::run_blocking;

pub struct FileSystemContentRepository {
    content_root: PathBuf,
    auto_reload: bool,
}

impl FileSystemContentRepository {
    pub fn new(content_root: impl Into<PathBuf>, auto_reload: bool) -> Self {
        Self {
            content_root: content_root.into(),
            auto_reload,
        }
    }
}

impl ContentRepository for FileSystemContentRepository {
    /// Dev (`auto_reload`) = `"<newest mtime ms>:<file count>"` over regular files with hidden
    /// subtrees pruned (`.git` churn must not bump it); an FS hiccup degrades to `"0:0"`.
    /// Prod = the checkout's HEAD SHA, re-read per call.
    async fn content_version(&self) -> String {
        let root = self.content_root.clone();
        let auto_reload = self.auto_reload;
        run_blocking(move || {
            if auto_reload {
                watermark(&root)
            } else {
                read_commit_sha(&root)
            }
        })
        .await
    }

    async fn load_tree(&self) -> Result<Vec<ContentEntry>, ContentError> {
        let root = self.content_root.clone();
        run_blocking(move || {
            if !root.is_dir() {
                return Ok(Vec::new());
            }
            let mut tree = Vec::new();
            for path in list_children(&root)? {
                if is_content_dir(&path) {
                    tree.push(load_dir(&path)?);
                }
            }
            Ok(tree)
        })
        .await
    }

    async fn read_lesson(&self, path: &str) -> Result<String, ContentError> {
        let root = self.content_root.clone();
        let rel = path.to_owned();
        run_blocking(move || {
            let target = safe_under_root(&root, &rel)?;
            fs::read_to_string(target).map_err(|e| ContentError::Io(e.to_string()))
        })
        .await
    }
}

fn load_dir(dir: &Path) -> Result<ContentEntry, ContentError> {
    let name = file_name(dir);
    let mut children = Vec::new();
    for path in list_children(dir)? {
        if is_content_dir(&path) {
            children.push(load_dir(&path)?);
        } else if is_markdown(&path) {
            let content = fs::read_to_string(&path).map_err(|e| ContentError::Io(e.to_string()))?;
            children.push(ContentEntry::File {
                name: file_name(&path),
                content,
            });
        }
    }
    Ok(ContentEntry::Dir {
        name,
        book_meta: read_json::<BookMeta>(&dir.join("book.json")),
        category_meta: read_json::<CategoryMeta>(&dir.join("category.json")),
        children,
    })
}

/// Sorted for determinism (the walker re-sorts by its own rules anyway).
fn list_children(dir: &Path) -> Result<Vec<PathBuf>, ContentError> {
    let entries = fs::read_dir(dir).map_err(|e| ContentError::Io(e.to_string()))?;
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
    paths.sort();
    Ok(paths)
}

fn is_content_dir(path: &Path) -> bool {
    path.is_dir() && !file_name(path).starts_with('.')
}

// Case-sensitive on purpose (oracle parity): content extensions are lowercase by convention.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn is_markdown(path: &Path) -> bool {
    path.is_file() && file_name(path).ends_with(".md")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Lenient marker decode (ADR-0001): not a file / unreadable / malformed → `None`.
fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Defense-in-depth under the service's slug check: the realpath of the resolved target must
/// stay under the realpath of the root AND be a regular file.
fn safe_under_root(root: &Path, rel: &str) -> Result<PathBuf, ContentError> {
    let denied = || ContentError::NotFound(format!("no content at '{rel}'"));
    let real_root = root.canonicalize().map_err(|_| denied())?;
    let target = root.join(rel).canonicalize().map_err(|_| denied())?;
    if target.starts_with(&real_root) && target.is_file() {
        Ok(target)
    } else {
        Err(denied())
    }
}

/// `"<newest mtime ms>:<regular file count>"`, hidden subtrees pruned; degrades to `"0:0"`.
fn watermark(root: &Path) -> String {
    fn scan(dir: &Path, newest: &mut u128, count: &mut u64) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if file_name(&path).starts_with('.') {
                continue;
            }
            if path.is_dir() {
                scan(&path, newest, count);
            } else if path.is_file() {
                *count += 1;
                if let Ok(modified) = path.metadata().and_then(|m| m.modified()) {
                    let ms = modified.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
                    *newest = (*newest).max(ms);
                }
            }
        }
    }
    let (mut newest, mut count) = (0_u128, 0_u64);
    if root.is_dir() {
        scan(root, &mut newest, &mut count);
    }
    format!("{newest}:{count}")
}
