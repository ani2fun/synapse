//! The catalog wire contract. Field names and the `kind` discriminator are LOAD-BEARING — this
//! is the JSON the client decodes. Tree nodes discriminate on `"kind"`: `"category"`/`"book"`
//! at library level, `"chapter"`/`"lesson"` inside a book. Options serialize as nulls, never
//! omitted.

use serde::{Deserialize, Serialize};

use crate::execution::TestSpec;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CatalogEntryDto {
    Category(CategoryDto),
    Book(BookDto),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub order: Option<i32>,
    #[cfg_attr(feature = "openapi", schema(no_recursion))]
    pub entries: Vec<CatalogEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BookDto {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub estimated_reading_minutes: Option<i32>,
    pub order: Option<i32>,
    pub category_path: Vec<String>,
    #[cfg_attr(feature = "openapi", schema(no_recursion))]
    pub entries: Vec<BookEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BookEntryDto {
    Chapter(ChapterDto),
    Lesson(LessonDto),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ChapterDto {
    pub slug: String,
    pub title: String,
    pub order: Option<i32>,
    #[cfg_attr(feature = "openapi", schema(no_recursion))]
    pub entries: Vec<BookEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LessonDto {
    pub slug: String,
    pub title: String,
    pub order: Option<i32>,
    pub essential: bool,
    /// The lesson's frontmatter `kind`, index-side, so the client can tell a problem from prose
    /// without fetching every payload (drives the "PROBLEM N / M" counter).
    ///
    /// NOT named `kind`: `BookEntryDto` is `#[serde(tag = "kind")]`, so a field by that name
    /// would emit the key TWICE — and serde cannot see inside the newtype to catch it. Skipped
    /// when absent, so prose lessons add nothing to a document every visitor downloads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lesson_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SynapseIndexDto {
    pub entries: Vec<CatalogEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BookRefDto {
    pub slug: String,
    pub title: String,
    pub category_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LessonFrontmatterDto {
    pub title: String,
    pub summary: Option<String>,
    pub essential: Option<bool>,
    pub kind: Option<String>,
    pub difficulty: Option<String>,
    pub topics: Option<Vec<String>>,
}

/// The lesson the reader renders. `raw` = the markdown body, fence stripped; `prev`/`next` are
/// ready-to-navigate FULL paths (`category…/book/chapter…/lesson`), null at book ends. `tests`
/// carries ONLY the sample cases for a `kind: problem` lesson (the workbench sources them from
/// here now that the description markdown no longer duplicates a `testcases` fence); null otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LessonPayloadDto {
    pub book: BookRefDto,
    pub lesson: LessonDto,
    pub frontmatter: LessonFrontmatterDto,
    pub raw: String,
    pub prev: Option<String>,
    pub next: Option<String>,
    pub editorial: Option<String>,
    pub tests: Option<TestSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ComponentDocDto {
    pub title: Option<String>,
    pub kind: Option<String>,
    pub technology: Option<String>,
    pub body: String,
}

/// A registered content repository, as the admin panel sees it. The sync fields are read-only —
/// they are the fetch loop's report, and they are what turns "I registered it" into "it landed".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ContentSourceDto {
    /// Derived from the repository name; names the checkout on disk and never changes.
    pub id: String,
    /// `owner/name`.
    pub repo: String,
    pub branch: String,
    /// `/`-joined category slug path; empty string is the top level.
    pub grouping: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,
    pub enabled: bool,
    /// The commit currently on disk. Absent until the first fetch lands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sha: Option<String>,
    /// ISO-8601 instant of the last fetch ATTEMPT, successful or not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<String>,
    /// Set when the last attempt failed. `lastSha` may still be present — a broken push degrades
    /// the book to stale, not to absent, and the panel should say which.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// One repository the merged LikeC4 workspace should pull `.c4` files from.
///
/// Deliberately unauthenticated and deliberately minimal: the consumer is a CI job with no token,
/// and every repository named here is already public. It carries no sync state and no grouping —
/// the build wants sources, not the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct C4SourceDto {
    /// `owner/name`.
    pub repo: String,
    pub branch: String,
}

/// Register or re-register a repository. Keyed server-side on the id derived from `repo`, so
/// posting the same repository twice edits its row rather than adding a second.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RegisterContentSourceDto {
    pub repo: String,
    /// Defaults to `main` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// `/`-joined category slug path; absent or empty is the top level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouping: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,
    /// Defaults to `true`. Registering disabled lets a repository be staged and verified before
    /// it joins the library.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn lesson(kind: Option<&str>) -> BookEntryDto {
        BookEntryDto::Lesson(LessonDto {
            slug: "two-sum".to_owned(),
            title: "Two Sum".to_owned(),
            order: Some(1),
            essential: true,
            lesson_kind: kind.map(str::to_owned),
        })
    }

    /// `BookEntryDto` is internally tagged on `kind`, so a `LessonDto` field named `kind` would
    /// serialize the key twice — silently, because the derive cannot see inside the newtype.
    /// This is the test that says the discriminator still means exactly one thing.
    #[test]
    fn the_lesson_kind_field_does_not_collide_with_the_enum_tag() {
        let json = serde_json::to_string(&lesson(Some("problem"))).unwrap();
        assert_eq!(json.matches("\"kind\":").count(), 1, "duplicate tag in {json}");
        assert!(json.contains(r#""kind":"lesson""#), "{json}");
        assert!(json.contains(r#""lessonKind":"problem""#), "{json}");
    }

    #[test]
    fn a_lesson_round_trips_through_the_wire() {
        let entry = lesson(Some("problem"));
        let json = serde_json::to_string(&entry).unwrap();
        assert_eq!(serde_json::from_str::<BookEntryDto>(&json).unwrap(), entry);
    }

    /// Prose is the common case and pays nothing: the key is absent, not null.
    #[test]
    fn prose_lessons_carry_no_kind_at_all() {
        let json = serde_json::to_string(&lesson(None)).unwrap();
        assert!(!json.contains("lessonKind"), "{json}");
        assert_eq!(serde_json::from_str::<BookEntryDto>(&json).unwrap(), lesson(None));
    }
}
