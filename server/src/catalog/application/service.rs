//! The catalog service — the driving use cases over the `ContentRepository` port, with the
//! version-gated index cache.

use std::sync::Arc;

use synapse_shared::execution::TestSpec;
use tokio::sync::{Mutex, RwLock};

use crate::catalog::application::content_repository::{ContentError, ContentRepository};
use crate::catalog::application::content_sources::Placements;
use crate::catalog::domain::catalog::{CatalogWarning, LessonFileRef, SynapseContentCatalog, WalkResult};
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

pub struct CatalogService<R> {
    repo: R,
    /// Where each satellite's book grafts. Shared with the sync loop, which republishes it as
    /// registrations change — a satellite's URL includes its grouping, so resolving without this
    /// would look the book up at the wrong path.
    placements: Placements,
    /// `(content version, snapshot)` — rebuilt only when the version moves.
    cache: RwLock<Option<(String, Arc<Snapshot>)>>,
    /// Held for the length of a rebuild, so only one runs at a time.
    ///
    /// It used to be absent, on the reasoning that a concurrent double rebuild is harmless because
    /// the walk is idempotent. Harmless for CORRECTNESS — but every reader arriving during one did
    /// its own full rebuild, reading every body in the catalog again. Measured: five concurrent
    /// readers after one invalidation cost five rebuilds, not one, and the pod has a single CPU.
    rebuilding: Mutex<()>,
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
            rebuilding: Mutex::new(()),
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
        if let Some(fresh) = self.cached(&version).await {
            return Ok(fresh);
        }

        // One rebuild at a time — and a caller that loses the race does NOT queue behind it. The
        // previous snapshot is still correct content, one version behind, and it is only ever the
        // TREE: titles, ordering, which lesson exists, and the search index. A lesson's body is
        // re-read per request whatever this returns, so what a stale reader misses is a rename or
        // a reordering, never the prose they came for.
        //
        // `try_lock`, therefore, not `lock`: queueing would trade one reader's wait for everyone's.
        let Ok(_guard) = self.rebuilding.try_lock() else {
            if let Some(stale) = self.any_cached().await {
                return Ok(stale);
            }
            // Cold start, and someone else is already building it: there is nothing to serve, so
            // this is the one case that waits. Re-read the version after the wait — the winner may
            // have built a different one, and caching under a version nobody asked for would make
            // the very next request miss.
            let _queued = self.rebuilding.lock().await;
            let version = self.repo.content_version().await;
            return match self.cached(&version).await {
                Some(built) => Ok(built),
                None => self.rebuild(version).await,
            };
        };

        // The winner may have finished between the miss above and the lock.
        if let Some(fresh) = self.cached(&version).await {
            return Ok(fresh);
        }
        self.rebuild(version).await
    }

    /// The snapshot for exactly this version, or nothing.
    async fn cached(&self, version: &str) -> Option<Arc<Snapshot>> {
        let held = self.cache.read().await;
        held.as_ref()
            .filter(|(cached, _)| cached == version)
            .map(|(_, snapshot)| Arc::clone(snapshot))
    }

    /// Whatever snapshot there is, however old — what a reader gets while someone else rebuilds.
    async fn any_cached(&self) -> Option<Arc<Snapshot>> {
        self.cache
            .read()
            .await
            .as_ref()
            .map(|(_, snapshot)| Arc::clone(snapshot))
    }

    /// Walk, merge and index every source, and publish the result. The caller holds `rebuilding`.
    async fn rebuild(&self, version: String) -> Result<Arc<Snapshot>, ContentError> {
        let t_load = std::time::Instant::now();
        let sources = self.repo.load_sources().await?;
        let load_ms = t_load.elapsed().as_millis();
        let placements = self.placements.snapshot();
        let t_walk = std::time::Instant::now();
        let walk = Arc::new(merge::assemble(&sources, &placements).map_err(ContentError::IndexInvalid)?);
        let walk_ms = t_walk.elapsed().as_millis();
        for warning in &walk.warnings {
            tracing::warn!(?warning, "catalog: cross-source conflict resolved");
        }
        // The last use of `sources`: every body is still in memory from the walk, and is dropped
        // with it on the next line.
        let t_index = std::time::Instant::now();
        let search = search::index_of(&sources, &walk);
        let index_ms = t_index.elapsed().as_millis();
        tracing::info!(documents = search.len(), %version, load_ms, walk_ms, index_ms, "catalog: search index built");
        let snapshot = Arc::new(Snapshot { walk, search });
        *self.cache.write().await = Some((version, Arc::clone(&snapshot)));
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests;
