//! Pure execution domain — the language model. No sandbox ids, no magic ints: languages are an
//! enum with labels and fence aliases (the code-quality bar's canonical "model it as an enum"
//! example).

mod language;

pub use language::Language;

/// The sandbox's hard edges — hardcoded, since go-judge exposes no runner-info endpoint to query
/// them from. Byte caps are UTF-8 byte counts, INCLUSIVE (`> limit` fails).
///
/// Lives here rather than in `synapse-shared`: it carries no serde and never crosses the wire,
/// so "shared" would describe the folder rather than the fact.
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
