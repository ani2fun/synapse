//! The authoring adapters: where the source comes from, where proposals are recorded, and the two
//! forges — the real one and the credential-free dry run that the whole flow is exercisable
//! against.

mod configured;
mod dry_run;
mod forges;
mod github;
mod github_wire;
mod lesson_source;
mod postgres;

pub use configured::ConfiguredForge;
pub use dry_run::DryRunForge;
pub use forges::ConfiguredForges;
pub use github::GitHubForge;
pub use lesson_source::FsLessonSource;
pub use postgres::{PostgresContentEditors, PostgresEditRequests};
