//! The catalog service — the driving use cases over the `ContentRepository` port, with the
//! version-gated index cache.

use std::sync::Arc;

use synapse_shared::execution::TestSpec;
use tokio::sync::RwLock;

use crate::catalog::application::content_repository::{ContentError, ContentRepository};
use crate::catalog::application::content_sources::Placements;
use crate::catalog::domain::catalog::{CatalogWarning, LessonFileRef, SynapseContentCatalog, WalkResult};
use crate::catalog::domain::component_doc::ComponentDoc;
use crate::catalog::domain::lesson::LessonContent;
use crate::catalog::domain::{frontmatter, merge, resolver, walker};

/// LikeC4 element ids: dotted FQNs of `[A-Za-z0-9_-]` segments.
fn element_id_like(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

pub struct CatalogService<R> {
    repo: R,
    /// Where each satellite's book grafts. Shared with the sync loop, which republishes it as
    /// registrations change — a satellite's URL includes its grouping, so resolving without this
    /// would look the book up at the wrong path.
    placements: Placements,
    /// `(content version, walk)` — rebuilt only when the version moves. A concurrent double
    /// rebuild is harmless because the walk is idempotent.
    cache: RwLock<Option<(String, Arc<WalkResult>)>>,
}

impl<R: ContentRepository> CatalogService<R> {
    /// The single-checkout deployment: no satellites, so no placements.
    pub fn new(repo: R) -> Self {
        Self::with_placements(repo, Placements::default())
    }

    pub fn with_placements(repo: R, placements: Placements) -> Self {
        Self {
            repo,
            placements,
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

        let source = self
            .repo
            .read_lesson(&file_path.source_id, &file_path.path)
            .await?;
        let parsed = frontmatter::parse(&source, &lesson.title);
        let editorial = self
            .editorial_for(file_path, parsed.frontmatter.kind.as_deref())
            .await?;
        let sample_tests = self
            .sample_tests_for(file_path, parsed.frontmatter.kind.as_deref())
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
            sample_tests,
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
        let sidecar = file_path.neighbour(&format!("_c4-docs/{leaf}.md"));
        let raw = self.repo.read_lesson(&sidecar.source_id, &sidecar.path).await?;
        Ok(ComponentDoc::parse(&raw))
    }

    /// `kind: problem` lessons may carry a `<lesson>.editorial.md` sidecar; its absence is
    /// normal, other repo failures propagate.
    async fn editorial_for(
        &self,
        lesson_file: &LessonFileRef,
        kind: Option<&str>,
    ) -> Result<Option<String>, ContentError> {
        if kind != Some("problem") {
            return Ok(None);
        }
        let editorial = lesson_file.sidecar(".editorial.md");
        match self.repo.read_lesson(&editorial.source_id, &editorial.path).await {
            Ok(text) => Ok(Some(text)),
            Err(ContentError::NotFound(_)) => Ok(None),
            Err(other) => Err(other),
        }
    }

    /// A `kind: problem` lesson's `<lesson>.tests.json` sidecar, projected to its SAMPLE cases —
    /// the only testcases the browser may see. The full suite stays server-side with the judge
    /// (`FsProblemTests` reads the same file for grading). Absent sidecar (or a non-problem lesson)
    /// → `None`; a malformed sidecar is a loud `Io` error, the same authoring bug the judge hits.
    async fn sample_tests_for(
        &self,
        lesson_file: &LessonFileRef,
        kind: Option<&str>,
    ) -> Result<Option<TestSpec>, ContentError> {
        if kind != Some("problem") {
            return Ok(None);
        }
        let tests = lesson_file.sidecar(".tests.json");
        match self.repo.read_lesson(&tests.source_id, &tests.path).await {
            Ok(text) => {
                let spec: TestSpec = serde_json::from_str(&text)
                    .map_err(|err| ContentError::Io(format!("invalid {}: {err}", tests.path)))?;
                Ok(Some(spec.samples()))
            }
            Err(ContentError::NotFound(_)) => Ok(None),
            Err(other) => Err(other),
        }
    }

    /// What the merge had to resolve across sources, for the admin panel.
    ///
    /// These are the signal that makes a content migration safe: while a book exists both in the
    /// spine and in its own repository, `DuplicateBookSlug` names which copy is actually serving.
    /// Until now that only reached the pod's log, where the person doing the migration is not
    /// looking. Costs a version check — the walk itself is the cached one every read already uses.
    pub async fn warnings(&self) -> Result<Vec<CatalogWarning>, ContentError> {
        Ok(self.current_walk().await?.warnings.clone())
    }

    /// The version-gated cache: hit iff the cached version equals the repo's current one.
    async fn current_walk(&self) -> Result<Arc<WalkResult>, ContentError> {
        let version = self.repo.content_version().await;
        if let Some((cached_version, walk)) = &*self.cache.read().await
            && *cached_version == version
        {
            return Ok(Arc::clone(walk));
        }
        let sources = self.repo.load_sources().await?;
        let placements = self.placements.snapshot();
        let walk = Arc::new(merge::assemble(&sources, &placements).map_err(ContentError::IndexInvalid)?);
        for warning in &walk.warnings {
            tracing::warn!(?warning, "catalog: cross-source conflict resolved");
        }
        *self.cache.write().await = Some((version, Arc::clone(&walk)));
        Ok(walk)
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
