//! The filesystem blog adapter (oracle: `FileSystemBlogRepository`): posts are `<slug>.md`
//! files directly under `<contentRoot>/blog/` — non-recursive, `_`-prefixed files are drafts
//! and never ship. Reads are traversal-guarded; the watermark mirrors the catalog's
//! dev-vs-prod split via `auto_reload`.

use std::path::{Path, PathBuf};

use crate::blog::application::{BlogError, BlogRepository};
use crate::platform::blocking::run_blocking;

pub struct FileSystemBlogRepository {
    root: PathBuf,
    auto_reload: bool,
}

impl FileSystemBlogRepository {
    pub fn new(content_root: impl AsRef<Path>, auto_reload: bool) -> Self {
        Self {
            root: content_root.as_ref().join("blog"),
            auto_reload,
        }
    }
}

impl BlogRepository for FileSystemBlogRepository {
    async fn version(&self) -> String {
        if !self.auto_reload {
            return "static".to_owned();
        }
        let root = self.root.clone();
        run_blocking(move || watermark(&root)).await
    }

    async fn load_all(&self) -> Result<Vec<(String, String)>, BlogError> {
        let root = self.root.clone();
        run_blocking(move || {
            let files = post_files(&root)?;
            let mut posts = Vec::with_capacity(files.len());
            for path in files {
                let slug = slug_of(&path);
                let body = std::fs::read_to_string(&path).map_err(|e| BlogError::Io(e.to_string()))?;
                posts.push((slug, body));
            }
            tracing::debug!(posts = posts.len(), "blog repo: loaded posts");
            Ok(posts)
        })
        .await
    }

    async fn read(&self, slug: &str) -> Result<String, BlogError> {
        if !slug_like(slug) {
            return Err(BlogError::NotFound(slug.to_owned()));
        }
        let root = self.root.clone();
        let slug = slug.to_owned();
        run_blocking(move || {
            let file = safe_file(&root, &slug)?;
            std::fs::read_to_string(file).map_err(|e| BlogError::Io(e.to_string()))
        })
        .await
    }
}

/// Slugs are plain identifiers — anything else (separators, dots) is `NotFound` by definition.
fn slug_like(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
}

/// Resolve `<root>/<slug>.md`, then verify the REAL path still lives under the real root and
/// is a regular file — the traversal guard.
fn safe_file(root: &Path, slug: &str) -> Result<PathBuf, BlogError> {
    let root_real = root
        .canonicalize()
        .map_err(|_| BlogError::NotFound(slug.to_owned()))?;
    let real = root
        .join(format!("{slug}.md"))
        .canonicalize()
        .map_err(|_| BlogError::NotFound(slug.to_owned()))?;
    if real.starts_with(&root_real) && real.is_file() {
        Ok(real)
    } else {
        Err(BlogError::NotFound(slug.to_owned()))
    }
}

/// The publishable posts: regular `*.md` directly under the blog dir, drafts (`_` prefix)
/// skipped, sorted by filename. An absent dir is an EMPTY blog, not an error.
fn post_files(root: &Path) -> Result<Vec<PathBuf>, BlogError> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(root).map_err(|e| BlogError::Io(e.to_string()))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|ext| ext == "md")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('_'))
        })
        .collect();
    files.sort();
    Ok(files)
}

fn slug_of(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned()
}

/// Dev watermark: `<newest mtime millis>:<post count>` — any edit or add/remove moves it.
/// Degraded filesystems report a constant, they never fail a request.
fn watermark(root: &Path) -> String {
    let Ok(files) = post_files(root) else {
        return "0:0".to_owned();
    };
    let newest = files
        .iter()
        .filter_map(|p| p.metadata().ok())
        .filter_map(|m| m.modified().ok())
        .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .max()
        .unwrap_or(0);
    format!("{newest}:{}", files.len())
}

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
