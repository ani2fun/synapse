//! Pure execution domain — the language model (oracle: `Language.scala`). No sandbox ids, no
//! magic ints: languages are an enum with labels and fence aliases (the code-quality bar's
//! canonical "model it as an enum" example).

mod language;

pub use language::Language;

/// The sandbox's hard edges (oracle: `BackendLimits.goJudge` — hardcoded, no runner-info
/// endpoint). Byte caps are UTF-8 byte counts, INCLUSIVE (`> limit` fails).
///
/// Lived in `synapse-shared` until step 59 — but it carries no serde and never crosses the
/// wire, so "shared" described the folder rather than the fact (the step-45 test, reapplied).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_stdout_bytes: usize,
    pub max_source_bytes: usize,
    pub max_stdin_bytes: usize,
    pub default_run_timeout_ms: u64,
}

pub const GO_JUDGE_LIMITS: Limits = Limits {
    max_stdout_bytes: 1024 * 1024,
    max_source_bytes: 64 * 1024,
    max_stdin_bytes: 16 * 1024,
    default_run_timeout_ms: 10_000,
};
