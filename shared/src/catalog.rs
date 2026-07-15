//! The catalog wire contract (oracle: `CatalogApi.scala`, ADR-S012 code-first). Field names and
//! the `kind` discriminator are LOAD-BEARING — this is the JSON the client decodes. Tree nodes
//! discriminate on `"kind"`: `"category"`/`"book"` at library level, `"chapter"`/`"lesson"`
//! inside a book. Options serialize as nulls (circe parity).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CatalogEntryDto {
    Category(CategoryDto),
    Book(BookDto),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub order: Option<i32>,
    #[schema(no_recursion)]
    pub entries: Vec<CatalogEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookDto {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub estimated_reading_minutes: Option<i32>,
    pub order: Option<i32>,
    pub category_path: Vec<String>,
    #[schema(no_recursion)]
    pub entries: Vec<BookEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BookEntryDto {
    Chapter(ChapterDto),
    Lesson(LessonDto),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDto {
    pub slug: String,
    pub title: String,
    pub order: Option<i32>,
    #[schema(no_recursion)]
    pub entries: Vec<BookEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LessonDto {
    pub slug: String,
    pub title: String,
    pub order: Option<i32>,
    pub essential: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SynapseIndexDto {
    pub entries: Vec<CatalogEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookRefDto {
    pub slug: String,
    pub title: String,
    pub category_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
/// ready-to-navigate FULL paths (`category…/book/chapter…/lesson`), null at book ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LessonPayloadDto {
    pub book: BookRefDto,
    pub lesson: LessonDto,
    pub frontmatter: LessonFrontmatterDto,
    pub raw: String,
    pub prev: Option<String>,
    pub next: Option<String>,
    pub editorial: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDocDto {
    pub title: Option<String>,
    pub kind: Option<String>,
    pub technology: Option<String>,
    pub body: String,
}
