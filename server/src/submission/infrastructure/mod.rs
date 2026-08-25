//! The submission adapters — the Postgres store, the allowlist, and the sidecar-or-fence
//! suite resolver.

mod allowlist;
mod postgres;
mod problem_suite;
mod progress_recorder;

pub use allowlist::PostgresSubmissionAllowlist;
pub use postgres::PostgresSubmissionRepository;
pub use problem_suite::FsProblemTests;
pub use progress_recorder::ProgressRecorderAdapter;
