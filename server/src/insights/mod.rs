//! Readership — a thin flat context (CLAUDE.md: "thin contexts flat"), deliberately
//! NOT folded into `catalog`.
//!
//! Catalog is a pure content-serving context whose single output port is a filesystem; giving
//! it a Postgres port would make it dual-store for a concern that is not content. And it is not
//! `submission` either, which is about the reader's code rather than their reading. Measurement
//! is its own capability, so it gets its own context — the same reasoning that made `blog` a
//! deliberate twin of `catalog` rather than a reuse of it.
//!
//! There is no `domain/` here on purpose: a view is a timestamp and a path. Nothing in this
//! context has behaviour worth modelling, so a domain layer would be ceremony.

pub mod http;
mod postgres;

pub use postgres::PostgresLessonViews;

/// The context's error. HTTP mapping (at `http/`): `StoreFailed` → 500. Recording is
/// fire-and-forget at the call site, so this is only ever surfaced by the admin read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InsightsError {
    #[error("insights store failed: {0}")]
    StoreFailed(String),
}

/// One lesson's readership, as counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonViewCount {
    pub lesson_path: String,
    pub views: i64,
    pub authed_views: i64,
    pub last_viewed: chrono::DateTime<chrono::Utc>,
}

/// Where readership lands (native AFIT + a generic router, per RS001 — nothing varies at
/// runtime, so `dyn` would be ceremony).
///
/// `record` returns a `Result` rather than swallowing, because the PORT should not decide the
/// policy: the catalog route is what chooses fire-and-forget, and a future caller that wants to
/// care about the failure can. See `catalog::http::routes::get_synapse_lesson`.
pub trait LessonViewStore: Send + Sync {
    fn record(
        &self,
        lesson_path: &str,
        authed: bool,
    ) -> impl Future<Output = Result<(), InsightsError>> + Send;

    /// Most-read first, capped. Ties break on the path so the order is total and the tests can
    /// pin it.
    fn top(&self, limit: i64) -> impl Future<Output = Result<Vec<LessonViewCount>, InsightsError>> + Send;
}
