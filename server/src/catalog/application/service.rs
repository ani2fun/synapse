//! The catalog service — the driving use cases over the `ContentRepository` port, with the
//! version-gated index cache.

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::catalog::application::content_repository::{ContentError, ContentRepository};
use crate::catalog::domain::catalog::SynapseContentCatalog;
use crate::catalog::domain::catalog::WalkResult;
use crate::catalog::domain::component_doc::ComponentDoc;
use crate::catalog::domain::lesson::LessonContent;
use crate::catalog::domain::{frontmatter, resolver, walker};

/// LikeC4 element ids: dotted FQNs of `[A-Za-z0-9_-]` segments.
fn element_id_like(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

pub struct CatalogService<R> {
    repo: R,
    /// `(content version, walk)` — rebuilt only when the version moves. A concurrent double
    /// rebuild is harmless because the walk is idempotent.
    cache: RwLock<Option<(String, Arc<WalkResult>)>>,
}

impl<R: ContentRepository> CatalogService<R> {
    pub fn new(repo: R) -> Self {
        Self {
            repo,
            cache: RwLock::new(None),
        }
    }

    /// The browsable index (cached per content version).
    pub async fn index(&self) -> Result<SynapseContentCatalog, ContentError> {
        Ok(self.current_walk().await?.catalog.clone())
    }

    /// Every lesson URL in the catalog, for the sitemap. Paths only — the sitemap needs no
    /// titles, and building them here would mean cloning strings the caller throws away.
    pub async fn all_lesson_paths(&self) -> Result<Vec<String>, ContentError> {
        let walk = self.current_walk().await?;
        let mut paths = Vec::new();
        for book in resolver::all_books(&walk.catalog) {
            let prefix = resolver::book_prefix(book);
            for (in_book, _) in resolver::lessons_in_reading_order(book) {
                paths.push(format!("{prefix}/{in_book}"));
            }
        }
        Ok(paths)
    }

    /// A lesson by its full slug path — the body is RE-READ every request (live edits show;
    /// only the index build is cached).
    #[tracing::instrument(name = "catalog.lesson", skip(self), fields(path = %path.join("/")))]
    pub async fn lesson(&self, path: &[String]) -> Result<LessonContent, ContentError> {
        if path.is_empty() || !path.iter().all(|s| walker::slug_like(s)) {
            return Err(ContentError::NotFound(format!(
                "no lesson at '{}'",
                path.join("/")
            )));
        }
        let walk = self.current_walk().await?;
        let (book, in_book_path, lesson) = resolver::resolve_lesson(&walk.catalog, path)
            .ok_or_else(|| ContentError::NotFound(format!("no lesson at '{}'", path.join("/"))))?;
        let file_path = walk
            .lesson_files
            .get(&book.slug)
            .and_then(|files| files.get(&in_book_path))
            .ok_or_else(|| ContentError::NotFound(format!("no source for '{in_book_path}'")))?;

        let source = self.repo.read_lesson(file_path).await?;
        let parsed = frontmatter::parse(&source, &lesson.title);
        let editorial = self
            .editorial_for(file_path, parsed.frontmatter.kind.as_deref())
            .await?;

        let reading_order = resolver::lessons_in_reading_order(book);
        let position = reading_order.iter().position(|(p, _)| *p == in_book_path);
        let prev_path = position
            .filter(|&i| i > 0)
            .map(|i| reading_order[i - 1].0.clone());
        let next_path = position
            .filter(|&i| i + 1 < reading_order.len())
            .map(|i| reading_order[i + 1].0.clone());

        Ok(LessonContent {
            book: book.clone(),
            lesson: lesson.clone(),
            frontmatter: parsed.frontmatter,
            raw: parsed.body,
            prev_path,
            next_path,
            editorial,
        })
    }

    /// A LikeC4 component's tutorial doc: the co-located `_c4-docs/<leaf>.md` sidecar next to
    /// the lesson, keyed by the FQN's LEAF segment (a container view's FQN and a sub-view's
    /// bare leaf resolve the same file). Re-read per request; absent → `NotFound` → 404.
    pub async fn component_doc(
        &self,
        lesson_path: &[String],
        element_id: &str,
    ) -> Result<ComponentDoc, ContentError> {
        if !element_id_like(element_id) {
            return Err(ContentError::NotFound(format!(
                "no component doc for '{element_id}'"
            )));
        }
        if lesson_path.is_empty() || !lesson_path.iter().all(|s| walker::slug_like(s)) {
            return Err(ContentError::NotFound(format!(
                "no lesson at '{}'",
                lesson_path.join("/")
            )));
        }
        let walk = self.current_walk().await?;
        let (book, in_book_path, _) = resolver::resolve_lesson(&walk.catalog, lesson_path)
            .ok_or_else(|| ContentError::NotFound(format!("no lesson at '{}'", lesson_path.join("/"))))?;
        let file_path = walk
            .lesson_files
            .get(&book.slug)
            .and_then(|files| files.get(&in_book_path))
            .ok_or_else(|| ContentError::NotFound(format!("no source for '{in_book_path}'")))?;

        let leaf = element_id.rsplit('.').next().unwrap_or(element_id);
        let sidecar = match file_path.rsplit_once('/') {
            Some((dir, _)) => format!("{dir}/_c4-docs/{leaf}.md"),
            None => format!("_c4-docs/{leaf}.md"),
        };
        let raw = self.repo.read_lesson(&sidecar).await?;
        Ok(ComponentDoc::parse(&raw))
    }

    /// `kind: problem` lessons may carry a `<lesson>.editorial.md` sidecar; its absence is
    /// normal, other repo failures propagate.
    async fn editorial_for(
        &self,
        lesson_file: &str,
        kind: Option<&str>,
    ) -> Result<Option<String>, ContentError> {
        if kind != Some("problem") {
            return Ok(None);
        }
        let stem = lesson_file.strip_suffix(".md").unwrap_or(lesson_file);
        match self.repo.read_lesson(&format!("{stem}.editorial.md")).await {
            Ok(text) => Ok(Some(text)),
            Err(ContentError::NotFound(_)) => Ok(None),
            Err(other) => Err(other),
        }
    }

    /// The version-gated cache: hit iff the cached version equals the repo's current one.
    async fn current_walk(&self) -> Result<Arc<WalkResult>, ContentError> {
        let version = self.repo.content_version().await;
        if let Some((cached_version, walk)) = &*self.cache.read().await
            && *cached_version == version
        {
            return Ok(Arc::clone(walk));
        }
        let tree = self.repo.load_tree().await?;
        let walk = Arc::new(walker::walk(&tree).map_err(ContentError::IndexInvalid)?);
        *self.cache.write().await = Some((version, Arc::clone(&walk)));
        Ok(walk)
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
