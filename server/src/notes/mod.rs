//! Problem notes — the Notes tab's markdown scratchpad, synced to the account. A thin flat context
//! like `canvas` and `progress`: a note is a user id, a problem path, some text and a timestamp,
//! and nothing about it has behaviour worth a domain layer.
//!
//! It is not `canvas`. A canvas entry is a SNAPSHOT the reader chose to keep, one of many; a note
//! is ONE living document per problem that each save replaces. Sharing a table would force one of
//! those two shapes onto the other.

pub mod http;
mod postgres;

pub use postgres::PostgresNoteStore;

use synapse_shared::notes::{NOTE_MAX_UTF16, NoteDto};

/// The context's error. HTTP mapping (at `http`): `TooLong` → 400, `StoreFailed` → 500.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotesError {
    #[error("a note holds at most {max} characters; this one has {len}")]
    TooLong { len: usize, max: usize },
    #[error("note store failed: {0}")]
    StoreFailed(String),
}

/// Refuse a note longer than the browser would have let the reader type. Counted in UTF-16 code
/// units because that is what the textarea's `maxLength` counts — counting `char`s instead would
/// accept notes the client can never produce and, for text outside the BMP, disagree with it.
pub fn check_length(text: &str) -> Result<(), NotesError> {
    let len = text.encode_utf16().count();
    if len > NOTE_MAX_UTF16 {
        return Err(NotesError::TooLong {
            len,
            max: NOTE_MAX_UTF16,
        });
    }
    Ok(())
}

/// Where notes land (native AFIT + a concrete adapter, per RS001 — nothing varies at runtime).
pub trait NoteStore: Send + Sync {
    /// The caller's note for one problem; `None` when they have never written one.
    fn get(
        &self,
        user_id: &str,
        lesson_path: &str,
    ) -> impl Future<Output = Result<Option<NoteDto>, NotesError>> + Send;

    /// Replace the note (insert or update) and hand back the stored row — `updated_at` is the
    /// store's clock, not the caller's. Empty `text` deletes the note and returns `None`.
    fn put(
        &self,
        user_id: &str,
        lesson_path: &str,
        text: &str,
    ) -> impl Future<Output = Result<Option<NoteDto>, NotesError>> + Send;

    /// Clear ALL of this user's notes, returning the row count removed.
    fn erase_all_for(&self, user_id: &str) -> impl Future<Output = Result<usize, NotesError>> + Send;
}

#[cfg(test)]
mod tests;
