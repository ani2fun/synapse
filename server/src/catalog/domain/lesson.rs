//! Lesson payloads (oracle: `LessonContent.scala`) — the typed frontmatter and the assembled
//! lesson the service hands to the HTTP layer.

use crate::catalog::domain::catalog::{Book, Lesson};

/// Typed frontmatter; `title` always resolves (fence → first H1 → humanized filename).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonFrontmatter {
    pub title: String,
    pub summary: Option<String>,
    pub essential: Option<bool>,
    pub kind: Option<String>,
    pub difficulty: Option<String>,
    pub topics: Option<Vec<String>>,
}

/// A parsed lesson source: frontmatter + the body with the fence stripped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub frontmatter: LessonFrontmatter,
    pub body: String,
}

/// The assembled lesson. `prev_path`/`next_path` are IN-BOOK slug-paths here — the HTTP layer
/// prepends `categoryPath + bookSlug` to make the wire's full paths. `editorial` joins for
/// `kind: problem` lessons with a `.editorial.md` sidecar (oracle step 16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonContent {
    pub book: Book,
    pub lesson: Lesson,
    pub frontmatter: LessonFrontmatter,
    pub raw: String,
    pub prev_path: Option<String>,
    pub next_path: Option<String>,
    pub editorial: Option<String>,
}
