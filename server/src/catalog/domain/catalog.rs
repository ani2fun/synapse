//! The browsable catalog — what the walker produces from the raw tree. Lesson BODIES are not
//! held here; each is read on demand per request. The walk result carries the slug-path →
//! file-path map the adapter resolves reads through.

use std::collections::BTreeMap;

/// A library-level node: a category groups further entries; a book holds the reading tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogEntry {
    Category(Category),
    Book(Book),
}

impl CatalogEntry {
    pub fn slug(&self) -> &str {
        match self {
            Self::Category(c) => &c.slug,
            Self::Book(b) => &b.slug,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub order: Option<i32>,
    /// Whether a `category.json` furnished this, as opposed to the title being humanised from the
    /// slug. Only a real declaration can contest another one — without this, two sources that
    /// merely happen to nest the same grouping look to the merge like two authors claiming it, and
    /// the resulting warning names a conflict that does not exist.
    pub declared: bool,
    pub entries: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub estimated_reading_minutes: Option<i32>,
    pub order: Option<i32>,
    /// Slugs of the categories above this book (roots have `[]`).
    pub category_path: Vec<String>,
    pub entries: Vec<BookEntry>,
}

/// A node inside a book: chapters nest (≤ `walker::MAX_CHAPTER_DEPTH`), lessons are leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookEntry {
    Chapter {
        slug: String,
        title: String,
        order: Option<i32>,
        entries: Vec<BookEntry>,
    },
    Lesson(Lesson),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lesson {
    pub slug: String,
    pub title: String,
    pub order: Option<i32>,
    pub essential: bool,
    /// Frontmatter `summary:`, carried for the server-rendered meta tags.
    ///
    /// INDEX-ONLY — deliberately absent from `LessonDto`. The client never needs it here: it
    /// already receives `frontmatter.summary` on the lesson payload it fetches anyway, so
    /// putting it on the index too would add 442 strings to a document every visitor downloads
    /// to buy nothing.
    pub description: Option<String>,
    /// Frontmatter `kind:` — `problem` or, for prose, absent.
    ///
    /// This one DOES cross to `LessonDto`, where `description` deliberately doesn't, and the
    /// difference is what the client can derive without it. A summary it already holds on the
    /// payload; "is this lesson a problem" it cannot know at all without asking for all 442.
    /// The counter on a problem page needs the whole chapter's shape at once, so the answer
    /// rides the index — and costs nothing on prose, which is most of them.
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SynapseContentCatalog {
    pub entries: Vec<CatalogEntry>,
}

/// Where a lesson's source file lives: which content source owns it, and the path within that
/// source's root (order prefixes and real folder names intact — that is what the adapter opens).
///
/// The source id travels WITH the path because a bare path is ambiguous once more than one
/// checkout is mounted: two books with the same interior layout would otherwise cross-serve each
/// other's bodies, editorials and judge suites. Deriving a sidecar therefore goes through
/// `sidecar`/`neighbour`, which carry the source across by construction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LessonFileRef {
    pub source_id: String,
    pub path: String,
}

impl LessonFileRef {
    pub fn new(source_id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            source_id: source_id.into(),
            path: path.into(),
        }
    }

    /// A stem sidecar: `…/two-sum.md` + `.tests.json` → `…/two-sum.tests.json`.
    #[must_use]
    pub fn sidecar(&self, suffix: &str) -> Self {
        let stem = self.path.strip_suffix(".md").unwrap_or(&self.path);
        Self::new(&self.source_id, format!("{stem}{suffix}"))
    }

    /// A file in the lesson's own directory: `a/b/x.md` + `_c4-docs/y.md` → `a/b/_c4-docs/y.md`.
    #[must_use]
    pub fn neighbour(&self, relative: &str) -> Self {
        match self.path.rsplit_once('/') {
            Some((dir, _)) => Self::new(&self.source_id, format!("{dir}/{relative}")),
            None => Self::new(&self.source_id, relative),
        }
    }
}

/// The walk's full output: the catalog plus, per book slug, the map from in-book lesson
/// slug-path to the source and file that back it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WalkResult {
    pub catalog: SynapseContentCatalog,
    pub lesson_files: BTreeMap<String, BTreeMap<String, LessonFileRef>>,
    /// Cross-source conflicts, as DATA rather than log lines. Two reasons: the domain stays free
    /// of `tracing`, and the walk runs uncached on every edit-source fetch and every submit — a
    /// warn inside it would fire per request instead of once per content version. The caller with
    /// the cache logs them; the admin panel shows them.
    pub warnings: Vec<CatalogWarning>,
}

/// A conflict the merge resolved rather than failed on. Within one source these would be errors;
/// across sources they are survivable, because refusing to serve the whole library over one
/// clashing satellite is worse than serving the winner and saying so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogWarning {
    /// Two sources claim one book slug. The earlier source wins — and since the primary checkout
    /// is always first, a book being migrated out of it keeps serving until it is deleted there.
    DuplicateBookSlug {
        slug: String,
        kept_source: String,
        skipped_source: String,
    },
    /// Two sources ship a `category.json` for the same slug. The first declaration wins; a
    /// category the merge SYNTHESIZED does not count as a declaration and is still upgradable.
    CategoryRedeclared {
        slug: String,
        kept_source: String,
        ignored_source: String,
    },
    /// A source whose root is a book carries no `slug` in `book.json`, so its URL fell back to the
    /// source id. Loud because the fallback silently moves every lesson in that book.
    BookSourceWithoutSlug { source_id: String },
}

#[cfg(test)]
mod tests;

/// Convention violations the walk refuses to paper over.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SynapseContentError {
    #[error("duplicate book slug: {0}")]
    DuplicateBookSlug(String),
    #[error("duplicate lesson slug-paths in book '{book_slug}': {slugs:?}")]
    DuplicateLessonSlug { book_slug: String, slugs: Vec<String> },
    #[error("chapter nesting exceeds the maximum at '{0}'")]
    MaxChapterDepthExceeded(String),
    #[error("invalid slug '{slug}' at '{path_in_book}'")]
    InvalidSlug { path_in_book: String, slug: String },
}
