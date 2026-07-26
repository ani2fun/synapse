//! The uninterpreted on-disk tree — what the filesystem adapter materializes and hands to the
//! walker. Metadata is pre-decoded (`book.json`/`category.json`); everything optional, lenient
//! by design (ADR-0001).

use serde::Deserialize;

/// The source id a single-checkout deployment walks under — the git-sync'd primary tree.
pub const PRIMARY_SOURCE_ID: &str = "main";

/// One content source's tree: a whole checkout, with the ROOT's own markers decoded.
///
/// The root markers are what tell the two source shapes apart. A checkout whose root carries a
/// `book.json` IS one book — a satellite guide repo, its chapters directly at the root. One
/// without is a collection, walked by directory nesting. The distinction has to live here because
/// the adapter only decodes markers one level down: hand the walker a bare list of the root's
/// children and the root's own `book.json` is unreachable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTree {
    /// Unique across sources — what a `LessonFileRef` points back through to reach the right root.
    pub id: String,
    pub book_meta: Option<BookMeta>,
    pub category_meta: Option<CategoryMeta>,
    pub children: Vec<ContentEntry>,
}

/// One entry of the raw content tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentEntry {
    /// A `.md` source; `name` keeps the `.md` suffix, `content` is the raw markdown.
    File { name: String, content: String },
    /// A directory, with whichever metadata markers it carried.
    Dir {
        name: String,
        book_meta: Option<BookMeta>,
        category_meta: Option<CategoryMeta>,
        children: Vec<ContentEntry>,
    },
}

impl ContentEntry {
    pub fn name(&self) -> &str {
        match self {
            Self::File { name, .. } | Self::Dir { name, .. } => name,
        }
    }
}

/// Decoded `book.json`. An explicit `slug` overrides the folder-derived one (file paths keep the
/// real folder name); `order` overrides the numeric prefix.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub estimated_reading_minutes: Option<i32>,
    pub order: Option<i32>,
    pub slug: Option<String>,
}

/// Decoded `category.json`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub order: Option<i32>,
    pub icon: Option<String>,
}
