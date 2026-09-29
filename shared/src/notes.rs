//! The problem-notes wire contract — the Notes tab's markdown scratchpad, one document per account
//! per problem. A save REPLACES the note rather than appending an entry: notes are what the reader
//! is thinking now, not a history of it (that is what saved canvas entries are for).

use serde::{Deserialize, Serialize};

/// The longest note the server stores, in UTF-16 code units — the unit the browser's `maxLength`
/// and `String.length` count, so a note the textarea accepted is never one the server refuses.
pub const NOTE_MAX_UTF16: usize = 10_000;

/// `PUT /api/notes` body. An empty `text` deletes the note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SaveNoteRequestDto {
    /// The problem's directory-mirror path, e.g. `["dsa", "arrays", "move-zeroes"]`.
    pub path: Vec<String>,
    pub text: String,
}

/// The caller's note for one problem. A problem with no note — or an anonymous caller — reads as
/// empty `text` with no `updatedAt`, so the client has one shape to handle rather than a 404 to
/// tell apart from a real failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub path: Vec<String>,
    pub text: String,
    /// ISO-8601 instant of the last save; absent when there is no stored note.
    pub updated_at: Option<String>,
}
