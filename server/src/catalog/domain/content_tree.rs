//! The uninterpreted on-disk tree — what the filesystem adapter materializes and hands to the
//! walker. Metadata is pre-decoded (`book.json`/`category.json`); everything optional, lenient
//! by design (ADR-0001).

use serde::Deserialize;

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
