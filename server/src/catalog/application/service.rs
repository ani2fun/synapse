//! The catalog service — the driving use cases over the `ContentRepository` port, with the
//! version-gated index cache.

use std::sync::Arc;

use synapse_shared::execution::TestSpec;
use tokio::sync::RwLock;

use crate::catalog::application::content_repository::{ContentError, ContentRepository};
use crate::catalog::application::content_sources::Placements;
use crate::catalog::domain::catalog::{CatalogWarning, LessonFileRef, SynapseContentCatalog, WalkResult};
use crate::catalog::domain::component_doc::ComponentDoc;
use crate::catalog::domain::d2_board::{BOARDS_DIR, BoardFile, D2Board};
use crate::catalog::domain::lesson::LessonContent;
use crate::catalog::domain::search::{SearchHit, SearchIndex};
use crate::catalog::domain::{frontmatter, merge, resolver, search, walker};

/// One content version's answers: the browsable tree and the searchable one.
///
/// They are cached TOGETHER on purpose. Two caches under two keys could serve a catalog from one
/// version and search results from another — a reader finding a lesson that the index they are
/// looking at says does not exist. One tuple, one version, no skew.
struct Snapshot {
    walk: Arc<WalkResult>,
    search: SearchIndex,
}

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
    /// `(content version, snapshot)` — rebuilt only when the version moves. A concurrent double
    /// rebuild is harmless because the walk is idempotent.
    cache: RwLock<Option<(String, Arc<Snapshot>)>>,
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
        Ok(self.current().await?.walk.catalog.clone())
    }

    /// Every lesson URL in the catalog, for the sitemap. Paths only — the sitemap needs no
    /// titles, and building them here would mean cloning strings the caller throws away.
    pub async fn all_lesson_paths(&self) -> Result<Vec<String>, ContentError> {
        let walk = Arc::clone(&self.current().await?.walk);
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
        let walk = Arc::clone(&self.current().await?.walk);
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
        let walk = Arc::clone(&self.current().await?.walk);
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

    /// One board of a `d2 boards` walkthrough: the co-located `_d2/<fence>/<file>` sidecar next
    /// to the lesson, drawn by the content repo's CI.
    ///
    /// `fence` and the file's stem are joined to a real filesystem path, so both are checked
    /// against `slug_like` rather than trusted, and `BoardFile` admits only a board or its
    /// manifest. Absent → `NotFound` → 404, exactly like a missing `_c4-docs` sidecar: a repo
    /// whose CI has not drawn its figures yet is a normal state, not an error.
    pub async fn d2_board(
        &self,
        lesson_path: &[String],
        fence: &str,
        file: &str,
    ) -> Result<D2Board, ContentError> {
        let missing = || ContentError::NotFound(format!("no board '{fence}/{file}'"));
        let (stem, kind) = BoardFile::parse(file).ok_or_else(missing)?;
        if !walker::slug_like(fence) || !walker::slug_like(stem) {
            return Err(missing());
        }
        if lesson_path.is_empty() || !lesson_path.iter().all(|s| walker::slug_like(s)) {
            return Err(ContentError::NotFound(format!(
                "no lesson at '{}'",
                lesson_path.join("/")
            )));
        }
        let walk = Arc::clone(&self.current().await?.walk);
        let (book, in_book_path, _) = resolver::resolve_lesson(&walk.catalog, lesson_path)
            .ok_or_else(|| ContentError::NotFound(format!("no lesson at '{}'", lesson_path.join("/"))))?;
        let file_path = walk
            .lesson_files
            .get(&book.slug)
            .and_then(|files| files.get(&in_book_path))
            .ok_or_else(|| ContentError::NotFound(format!("no source for '{in_book_path}'")))?;

        let sidecar = file_path.neighbour(&format!("{BOARDS_DIR}/{fence}/{file}"));
        let body = self.repo.read_lesson(&sidecar.source_id, &sidecar.path).await?;
        Ok(D2Board { file: kind, body })
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
        Ok(self.current().await?.walk.warnings.clone())
    }

    /// Ranked full-text hits across every mounted source.
    ///
    /// Reads the same version-gated snapshot everything else does, so search can never answer
    /// from a different content version than the page the reader lands on.
    #[tracing::instrument(name = "catalog.search", skip(self), fields(hits))]
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, ContentError> {
        let hits = self.current().await?.search.search(query, limit);
        tracing::Span::current().record("hits", hits.len());
        Ok(hits)
    }

    /// The version-gated cache: hit iff the cached version equals the repo's current one.
    ///
    /// The index is built HERE rather than inside `merge::assemble`, though the bodies are in
    /// hand at both points. `assemble` is also called uncached on every edit-source request and
    /// by the `validate_book` CLI, neither of which searches anything — building there would tax
    /// the editor to serve the palette.
    async fn current(&self) -> Result<Arc<Snapshot>, ContentError> {
        let version = self.repo.content_version().await;
        if let Some((cached_version, snapshot)) = &*self.cache.read().await
            && *cached_version == version
        {
            return Ok(Arc::clone(snapshot));
        }
        let sources = self.repo.load_sources().await?;
        let placements = self.placements.snapshot();
        let walk = Arc::new(merge::assemble(&sources, &placements).map_err(ContentError::IndexInvalid)?);
        for warning in &walk.warnings {
            tracing::warn!(?warning, "catalog: cross-source conflict resolved");
        }
        // The last use of `sources`: every body is still in memory from the walk, and is dropped
        // with it on the next line.
        let search = search::index_of(&sources, &walk);
        tracing::info!(documents = search.len(), %version, "catalog: search index built");
        let snapshot = Arc::new(Snapshot { walk, search });
        *self.cache.write().await = Some((version, Arc::clone(&snapshot)));
        Ok(snapshot)
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
