//! The Algorithm Design Canvas wire contract — the reader's PLAN for a problem, saved as
//! timestamped entries the way `submission` saves their code.
//!
//! The body is TYPED rather than opaque JSON. `algorithm-design-canvas/v1` is an export format
//! readers hand to other tools, so what is in an entry is part of the published contract and not
//! a shape the client happens to have written that day.
//!
//! Nothing derivable travels or is stored: an entry's title (the first line of `problem`), its
//! filled-area count and its best complexity are all computed from the body by whoever renders
//! them. A carried copy is a copy that can disagree with the body it describes.

use serde::{Deserialize, Serialize};

/// One approach on the canvas — brute force first, then refined. `time`/`space` are the author's
/// own Big-O strings, not parsed: "O(n log n)", "O(n) but only if the input is sorted" and "?"
/// are all things a reader legitimately writes while still thinking.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CanvasIdeaDto {
    pub name: String,
    pub description: String,
    pub time: String,
    pub space: String,
}

/// The eight areas of the canvas. Every field defaults to empty — a half-filled canvas is the
/// normal state of one being worked on, and a partial save must never be a 400.
///
/// `ret` rather than `return`, which is a Rust keyword; it is `return` on the wire.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CanvasBodyDto {
    #[serde(default)]
    pub problem: String,
    #[serde(default)]
    pub constraints: String,
    #[serde(default)]
    pub maintenance: String,
    #[serde(default)]
    pub inputs: String,
    #[serde(default, rename = "return")]
    pub ret: String,
    #[serde(default)]
    pub errors: String,
    #[serde(default)]
    pub tests: String,
    #[serde(default)]
    pub ideas: Vec<CanvasIdeaDto>,
}

/// `POST /api/canvas` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SaveCanvasRequestDto {
    /// The problem's directory-mirror path, e.g. `["dsa", "arrays", "move-zeroes"]`.
    pub path: Vec<String>,
    pub body: CanvasBodyDto,
}

/// One saved entry. The list ships full bodies, so opening an entry needs no second request —
/// the shape `SubmissionDto` uses for `source`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CanvasEntryDto {
    pub id: String,
    pub path: Vec<String>,
    pub body: CanvasBodyDto,
    /// ISO-8601 instant.
    pub created_at: String,
}
