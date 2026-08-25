//! Where an editable lesson's source comes from (`FsLessonSource`) — resolved THROUGH the catalog
//! walker's lesson-file map, never by joining the URL path onto the content root. Real folders
//! carry `NN-` order prefixes (`/foundations/` is the directory `01-foundations`), so naive
//! joining cannot work.
//!
//! Routing every edit through the catalog's own resolver has a second effect worth naming: only
//! files the catalog already SERVES are reachable. `local-only/`, `_`-prefixed files and the
//! reserved aux dirs are excluded by the walker, so they are structurally uneditable here — the
//! protection is the same one that keeps them unservable, not a second list to keep in sync.
//!
//! The file comes back WHOLE, frontmatter fence included. The reader's payload carries the body
//! with the fence stripped; saving that back would delete the frontmatter.

use crate::authoring::application::{AuthoringError, LessonFile, LessonSource};
use crate::catalog::application::{ContentError, ContentRepository, Placements};
use crate::catalog::domain::{merge, resolver, walker};

pub struct FsLessonSource<R> {
    repo: R,
    /// The same handle the catalog resolves through: a satellite's lesson path includes the
    /// grouping its placement gives it, so resolving without this finds nothing.
    placements: Placements,
}

impl<R> FsLessonSource<R> {
    pub fn new(repo: R) -> Self {
        Self::with_placements(repo, Placements::default())
    }

    pub fn with_placements(repo: R, placements: Placements) -> Self {
        Self { repo, placements }
    }
}

impl<R: ContentRepository> LessonSource for FsLessonSource<R> {
    async fn content_version(&self) -> String {
        self.repo.content_version().await
    }

    async fn file_for(&self, lesson_path: &[String]) -> Result<Option<LessonFile>, AuthoringError> {
        if lesson_path.is_empty() || !lesson_path.iter().all(|s| walker::slug_like(s)) {
            return Ok(None);
        }
        let sources = self.repo.load_sources().await.map_err(|e| unreadable(&e))?;
        let walk = merge::assemble(&sources, &self.placements.snapshot())
            .map_err(|error| AuthoringError::ContentUnreadable(format!("catalog index invalid: {error}")))?;
        let Some((book, in_book_path, _)) = resolver::resolve_lesson(&walk.catalog, lesson_path) else {
            return Ok(None);
        };
        let Some(file_path) = walk
            .lesson_files
            .get(&book.slug)
            .and_then(|files| files.get(&in_book_path))
        else {
            return Ok(None);
        };
        if is_local_only(&file_path.path) {
            // Reachable only under `render-local-only`, which puts study material in the catalog
            // for a local reader. Editing it would COMMIT a gitignored file — the one action that
            // turns "kept for private study" into "published", and the exact outcome ADR-RS002
            // exists to prevent. Unconditional here rather than behind the same feature: a guard
            // that only exists in the build that needs it is one refactor from being absent.
            tracing::warn!(path = %file_path.path, "edit refused — local-only content is never editable");
            return Ok(None);
        }
        // Re-read rather than reuse the tree's copy: the walk may be a moment old, and the
        // fingerprint the editor is handed must describe the bytes on disk right now.
        match self.repo.read_lesson(&file_path.source_id, &file_path.path).await {
            Ok(source) => Ok(Some(LessonFile {
                source_id: file_path.source_id.clone(),
                file_path: file_path.path.clone(),
                source,
            })),
            Err(ContentError::NotFound(_)) => Ok(None),
            Err(error) => Err(unreadable(&error)),
        }
    }
}

/// The gitignored study trees, under either spelling, order prefix or not.
fn is_local_only(path: &str) -> bool {
    path.split('/')
        .next()
        .map(walker::strip_order_prefix)
        .is_some_and(|head| head == "local-only-content" || head == "local-only")
}

fn unreadable(error: &ContentError) -> AuthoringError {
    AuthoringError::ContentUnreadable(error.to_string())
}

#[cfg(test)]
mod tests;
