//! Per-user completion progress — a thin flat context (CLAUDE.md: "thin contexts flat"). There is
//! no `domain/` here on purpose: a completion is a user id, a lesson path, and a timestamp; nothing
//! has behaviour worth modelling, so a domain layer would be ceremony.
//!
//! It is deliberately NOT `submission` (which is about the reader's CODE) nor `insights` (which is
//! anonymous content POPULARITY, with no user id by design). This is the account's own ✓ ticks,
//! keyed by the Keycloak `sub` — the same value `submissions.user_id` stores — and it owns its own
//! Postgres port so neither of those contexts becomes dual-store for a concern that is not theirs.

pub mod http;
mod postgres;

pub use postgres::PostgresProblemProgress;

/// The context's error. HTTP mapping (at `http/`): `StoreFailed` → 500. `mark` is fire-and-forget
/// at the solve call site, so a store hiccup there is swallowed; `list`/`reset` surface it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProgressError {
    #[error("progress store failed: {0}")]
    StoreFailed(String),
}

/// Where completion lands (native AFIT + a concrete adapter, per RS001 — nothing varies at
/// runtime, so `dyn` would be ceremony).
pub trait ProblemProgressStore: Send + Sync {
    /// Record one lesson as complete for a user. IDEMPOTENT: re-marking the same lesson is a no-op,
    /// so a re-read or a second accepted submission never errors or duplicates.
    fn mark(
        &self,
        user_id: &str,
        lesson_path: &str,
    ) -> impl Future<Output = Result<(), ProgressError>> + Send;

    /// Forget one lesson's completion — `mark`'s exact inverse, and idempotent in the same way:
    /// unmarking a lesson that was never marked is not an error, it is already the desired state.
    ///
    /// Per-lesson rather than only `reset_for`, because a reader who marks a page by mistake needs
    /// a way back that is not "clear everything". Without it the reader's ✓ is one-way, and a
    /// client that hid the tick locally would watch the next sign-in restore it.
    fn unmark(
        &self,
        user_id: &str,
        lesson_path: &str,
    ) -> impl Future<Output = Result<(), ProgressError>> + Send;

    /// Every lesson path the user has completed. Ordered by path so the result is total and the
    /// tests can pin it.
    fn list_for(&self, user_id: &str) -> impl Future<Output = Result<Vec<String>, ProgressError>> + Send;

    /// Clear ALL of the user's completion, returning the row count removed. Submissions are a
    /// separate store and are never touched — "reset progress" is not "erase my data".
    fn reset_for(&self, user_id: &str) -> impl Future<Output = Result<usize, ProgressError>> + Send;
}
