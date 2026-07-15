//! The browsable catalog (oracle: `SynapseContentCatalog.scala`) — what the walker produces from
//! the raw tree. Lesson BODIES are not held here (read on demand per request, ADR-S010); the
//! walk result carries the slug-path → file-path map the adapter resolves reads through.

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
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SynapseContentCatalog {
    pub entries: Vec<CatalogEntry>,
}

/// The walk's full output: the catalog plus, per book slug, the map from in-book lesson
/// slug-path to the content-root-relative file path (order prefixes and real folder names
/// intact — that is what the adapter opens).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WalkResult {
    pub catalog: SynapseContentCatalog,
    pub lesson_files: BTreeMap<String, BTreeMap<String, String>>,
}

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
