//! The execution wire contract.
//! `POST /api/run` speaks exactly these shapes; `RunStatus` crosses the wire as the CASE NAME
//! string — never a Judge0-style magic int (the code-quality bar's canonical example).

mod test_run;

pub use test_run::{ArgSpec, TestCase, TestSpec, Verdict, judge, stdin_for};

use serde::{Deserialize, Serialize};

/// What a run produced, as vocabulary. A badly-running program is still a 200 with a
/// non-`Accepted` status — only backend machinery failures use the error channel.
//
// Plain comment, not doc: utoipa lifts `///` into the OpenAPI description and from there into
// web/src/lib/api/schema.gen.ts, and none of the below is a wire consumer's business.
//
// THESE VARIANT NAMES ARE ALSO A STORAGE FORMAT. A completed submission persists its
// `SuiteOutcome` into the `outcome jsonb` column, and a rejection carries the failing case's
// status inside it as the bare variant name (`…firstFailure.status = "Accepted"`). So renaming
// one is a data migration over every historical row, not an API tweak — old rows would decode as
// an unknown variant and the submission would stop reading back.
//
// The INBOUND side is deliberately not coupled this way: go-judge's own status strings are
// translated by an explicit match in `execution::infrastructure::wire`, so the sandbox's
// vocabulary can change without touching this type, and this type can gain a variant without the
// sandbox knowing.
/// The wire form is the CASE NAME — never an index, so adding a variant cannot renumber the
/// others, and a payload stays readable without a lookup table.
///
/// ```
/// use synapse_shared::execution::RunStatus;
///
/// let json = serde_json::to_string(&RunStatus::TimeLimitExceeded).unwrap();
/// assert_eq!(json, "\"TimeLimitExceeded\"");
/// assert_eq!(serde_json::from_str::<RunStatus>(&json).unwrap(), RunStatus::TimeLimitExceeded);
///
/// // Only one status is a pass; every other carries a human label for the reader.
/// assert!(RunStatus::Accepted.is_success());
/// assert!(!RunStatus::CompileError.is_success());
/// assert_eq!(RunStatus::CompileError.label(), "Compilation Error");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum RunStatus {
    Accepted,
    CompileError,
    RuntimeError,
    TimeLimitExceeded,
    InternalError,
}

impl RunStatus {
    pub fn is_success(self) -> bool {
        self == Self::Accepted
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Accepted => "Accepted",
            Self::CompileError => "Compilation Error",
            Self::RuntimeError => "Runtime Error",
            Self::TimeLimitExceeded => "Time Limit Exceeded",
            Self::InternalError => "Internal Error",
        }
    }
}

/// The run request. `language` is a fence alias (`py`, `cpp`, …), resolved server-side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RunRequest {
    pub language: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
}

/// The run's outcome. `time_seconds`/`memory_kb` are absent when the backend didn't measure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub status: RunStatus,
    pub stdout: String,
    pub stderr: String,
    pub compile_output: String,
    pub time_seconds: Option<f64>,
    pub memory_kb: Option<i64>,
}

// NOTE: the sandbox `Limits` + `GO_JUDGE_LIMITS` live in `server::execution::domain`, not
// here — they carry no serde and never cross the wire, so keeping them out of this crate
// keeps it a pure wire kernel.

#[cfg(test)]
mod tests;
