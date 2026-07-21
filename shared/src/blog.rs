//! The blog wire contract. Field names are LOAD-BEARING. `publishedAt` is an ISO date STRING —
//! empty when the post is undated (deliberately non-`Option`; the card renders "" as nothing).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BlogSummaryDto {
    pub slug: String,
    pub title: String,
    pub summary: Option<String>,
    pub published_at: String,
    pub tags: Vec<String>,
    pub read_minutes: Option<i32>,
    pub eyebrow: Option<String>,
}

/// One post: the summary fields + the markdown body + publish-order neighbours
/// (`prev` = older, `next` = newer; null at the ends).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BlogPostDto {
    pub slug: String,
    pub title: String,
    pub summary: Option<String>,
    pub published_at: String,
    pub tags: Vec<String>,
    pub read_minutes: Option<i32>,
    pub eyebrow: Option<String>,
    pub body: String,
    pub prev: Option<String>,
    pub next: Option<String>,
}
