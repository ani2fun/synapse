//! The filesystem `ContentRepository` — walks every mounted source root, decodes `book.json`/
//! `category.json` leniently at every level INCLUDING each root itself (a root marker is what
//! makes a checkout one book rather than a collection), guards lesson reads against traversal
//! (realpaths BOTH sides — macOS `/tmp` is a symlink), and produces the content version (dev
//! watermark / prod git SHA), joined across sources.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::UNIX_EPOCH;

use serde::de::DeserializeOwned;

use crate::catalog::application::{ContentError, ContentRepository};
use crate::catalog::domain::content_tree::{
    BookMeta, CategoryMeta, ContentEntry, PRIMARY_SOURCE_ID, SourceTree,
};
use crate::catalog::infrastructure::commit_sha::read_commit_sha;
use crate::platform::blocking::run_blocking;

/// One mounted checkout: the id lessons are addressed through, and where it lives on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRoot {
    pub id: String,
    pub root: PathBuf,
}

impl SourceRoot {
    pub fn new(id: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            id: id.into(),
            root: root.into(),
        }
    }
}

/// What is mounted right now, swappable while the server runs.
///
/// The registry lives in Postgres and satellites are registered from the admin panel, so the
/// mounted set is not knowable at construction: the sync loop republishes it as repositories are
/// registered, disabled, or first land on disk. A plain `Vec` would have frozen the library at
/// boot and made every registration a redeploy — the exact thing the registry exists to avoid.
///
/// Order is the publisher's and stays load-bearing: primary first, satellites after, because that
/// is what decides the merge's first-wins rule.
#[derive(Clone, Default)]
pub struct MountedSources {
    inner: Arc<RwLock<Vec<SourceRoot>>>,
}

impl MountedSources {
    #[must_use]
    pub fn new(sources: Vec<SourceRoot>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(sources)),
        }
    }

    /// A poisoned lock degrades to "nothing mounted" rather than panicking a request: the writer
    /// is a background loop, and one failed publish must not take the reader down.
    #[must_use]
    pub fn snapshot(&self) -> Vec<SourceRoot> {
        self.inner.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn publish(&self, sources: Vec<SourceRoot>) {
        if let Ok(mut held) = self.inner.write() {
            *held = sources;
        }
    }
}

pub struct FileSystemContentRepository {
    sources: MountedSources,
    auto_reload: bool,
}

impl FileSystemContentRepository {
    /// The single-checkout deployment: one source, mounted as the primary.
    pub fn new(content_root: impl Into<PathBuf>, auto_reload: bool) -> Self {
        Self::over(
            vec![SourceRoot::new(PRIMARY_SOURCE_ID, content_root)],
            auto_reload,
        )
    }

    pub fn over(sources: Vec<SourceRoot>, auto_reload: bool) -> Self {
        Self::mounted(MountedSources::new(sources), auto_reload)
    }

    /// Share a mounted set with the sync loop that republishes it.
    pub fn mounted(sources: MountedSources, auto_reload: bool) -> Self {
        Self { sources, auto_reload }
    }

    fn root_of(&self, source_id: &str) -> Option<PathBuf> {
        self.sources
            .snapshot()
            .into_iter()
            .find(|s| s.id == source_id)
            .map(|s| s.root)
    }
}

impl ContentRepository for FileSystemContentRepository {
    /// Per source, dev (`auto_reload`) = `"<newest mtime ms>:<file count>"` over regular files
    /// with hidden subtrees pruned (`.git` churn must not bump it), an FS hiccup degrading to
    /// `"0:0"`; prod = that checkout's HEAD SHA, re-read per call. Joined `id=version`, so any
    /// one source moving invalidates the index.
    async fn content_version(&self) -> String {
        let sources = self.sources.snapshot();
        let auto_reload = self.auto_reload;
        run_blocking(move || {
            sources
                .iter()
                .map(|source| {
                    let version = if auto_reload {
                        watermark(&source.root)
                    } else {
                        read_commit_sha(&source.root)
                    };
                    format!("{}={version}", source.id)
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .await
    }

    async fn load_sources(&self) -> Result<Vec<SourceTree>, ContentError> {
        let sources = self.sources.snapshot();
        run_blocking(move || {
            let mut trees = Vec::with_capacity(sources.len());
            for source in sources {
                trees.push(load_source(&source)?);
            }
            Ok(trees)
        })
        .await
    }

    async fn read_lesson(&self, source_id: &str, path: &str) -> Result<String, ContentError> {
        let Some(root) = self.root_of(source_id) else {
            return Err(ContentError::NotFound(format!("no content source '{source_id}'")));
        };
        let rel = path.to_owned();
        run_blocking(move || {
            let target = safe_under_root(&root, &rel)?;
            fs::read_to_string(target).map_err(|e| ContentError::Io(e.to_string()))
        })
        .await
    }
}

/// A missing root is an EMPTY source, not an error: a satellite whose fetch has not landed yet
/// must not take the whole catalog down with it.
fn load_source(source: &SourceRoot) -> Result<SourceTree, ContentError> {
    let mut tree = SourceTree {
        id: source.id.clone(),
        book_meta: read_json(&source.root.join("book.json")),
        category_meta: read_json(&source.root.join("category.json")),
        children: Vec::new(),
    };
    if !source.root.is_dir() {
        return Ok(tree);
    }
    for path in list_children(&source.root)? {
        if is_content_dir(&path) {
            tree.children.push(load_dir(&path)?);
        } else if is_markdown(&path) {
            // Root-level markdown matters when the root IS a book: `index.md` is that book's
            // opening lesson. A collection root ignores loose files, so carrying them costs it
            // nothing — the walker decides, not the adapter.
            let content = fs::read_to_string(&path).map_err(|e| ContentError::Io(e.to_string()))?;
            tree.children.push(ContentEntry::File {
                name: file_name(&path),
                content,
            });
        }
    }
    Ok(tree)
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

// Case-sensitive on purpose: content extensions are lowercase by convention.
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
