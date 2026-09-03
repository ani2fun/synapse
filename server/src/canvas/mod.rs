//! Saved Algorithm Design Canvas entries — the reader's PLAN for a problem, stored the way
//! `submission` stores their CODE for it. A thin flat context (CLAUDE.md: "thin contexts flat"):
//! there is no `domain/` here on purpose, because an entry is a user id, a problem path, an
//! authored document and a timestamp. Nothing has behaviour worth modelling, so a domain layer
//! would be ceremony — the `progress` context's reasoning, unchanged.
//!
//! It is deliberately NOT `submission`: a canvas is what the reader decided BEFORE writing code,
//! it is never judged, and it survives independently of whether they ever submitted. Nor is it
//! `progress`, which is a ✓ tick with no content. It owns its own Postgres port so neither of
//! those becomes dual-store for a concern that is not theirs.

pub mod http;
mod postgres;

pub use postgres::PostgresCanvasStore;

use synapse_shared::canvas::CanvasEntryDto;

/// The context's error. HTTP mapping (at `http`): `StoreFailed` → 500, `NotFound`/`NotYours` →
/// 404/403. The two access errors are SEPARATE variants rather than one `Denied`, because the
/// edge tells a reader "no such entry" and "not yours" differently and should not have to guess
/// which happened from a string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CanvasError {
    #[error("canvas store failed: {0}")]
    StoreFailed(String),
    #[error("no canvas entry {0}")]
    NotFound(String),
    #[error("canvas entry {0} belongs to someone else")]
    NotYours(String),
}

/// Where saved canvases land (native AFIT + a concrete adapter, per RS001 — nothing varies at
/// runtime, so `dyn` would be ceremony).
pub trait CanvasStore: Send + Sync {
    /// Store one entry and hand back the stored row — the id and `created_at` are the store's to
    /// mint, so the caller learns them from the reply rather than predicting them.
    fn save(
        &self,
        user_id: &str,
        lesson_path: &str,
        body: &synapse_shared::canvas::CanvasBodyDto,
    ) -> impl Future<Output = Result<CanvasEntryDto, CanvasError>> + Send;

    /// This user's entries for ONE problem, newest first. Full bodies: opening a saved entry
    /// must not cost a second round trip.
    fn list_for(
        &self,
        user_id: &str,
        lesson_path: &str,
    ) -> impl Future<Output = Result<Vec<CanvasEntryDto>, CanvasError>> + Send;

    /// Delete one entry the user owns. `NotFound` when there is no such row and `NotYours` when
    /// there is one but it is another account's — never "deleted" for either, so a stranger
    /// cannot use the reply to discover that an id exists.
    fn delete(&self, user_id: &str, id: &str) -> impl Future<Output = Result<(), CanvasError>> + Send;

    /// Clear ALL of this user's entries, returning the row count removed. Submissions and
    /// progress are separate stores and are never touched.
    fn erase_all_for(&self, user_id: &str) -> impl Future<Output = Result<usize, CanvasError>> + Send;
}
