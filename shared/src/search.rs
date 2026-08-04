//! The search wire contract — full-text hits across the merged library.
//!
//! Its own module rather than a corner of `catalog`: the browsable index and the searchable one
//! answer different questions, and the index is a document every visitor downloads while these
//! are fetched per query.

use serde::{Deserialize, Serialize};

/// `GET /api/synapse/search` — ranked hits, best first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SearchResultsDto {
    /// Echoed back so a client can drop a reply that a later keystroke has already outdated.
    pub query: String,
    pub results: Vec<SearchHitDto>,
}

/// One hit. No score: the ORDER is the contract, and a float the client must ignore is noise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub title: String,
    /// Ancestor titles, outermost first — the category and book to show under the title.
    pub breadcrumb: Vec<String>,
    /// The lesson's URL path, no leading slash.
    pub path: String,
    /// `lesson`, or `editorial` for a problem's solution walkthrough — which a reader must be able
    /// to recognise before opening it, because it spoils the exercise.
    pub kind: String,
    pub book_slug: String,
    /// A quote from the prose, pre-split into matched and unmatched runs.
    pub snippet: Vec<SnippetSegmentDto>,
}

/// One run of a snippet.
///
/// Segments rather than offsets because Rust indexes strings by UTF-8 byte and JavaScript by
/// UTF-16 code unit: a range computed on the server highlights the wrong span in the browser the
/// moment the prose stops being ASCII. Pre-split runs also mean the client builds text nodes
/// instead of parsing markup, so a query containing `<script>` is inert by construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SnippetSegmentDto {
    pub text: String,
    pub marked: bool,
}
