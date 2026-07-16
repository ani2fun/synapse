//! Pure submission domain (oracle: `Submission.scala` + `SuiteOutcome.scala`). The state is an
//! ADT, not a status column with nullables — a verdict on a pending row or a completed row
//! without one is UNREPRESENTABLE in memory; Postgres flattens at the edge only.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use synapse_shared::execution::RunStatus;
use uuid::Uuid;

/// Newtype over UUID — submission ids never mix with other UUIDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubmissionId(pub Uuid);

impl std::fmt::Display for SubmissionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Submission {
    pub id: SubmissionId,
    /// The catalog directory-mirror path, e.g. `["dsa", "arrays", "move-zeroes"]`.
    pub lesson_path: Vec<String>,
    /// The fence alias as submitted — resolved by the EXECUTION context, not here.
    pub language: String,
    pub source: String,
    /// The anonymous seam: `None` until identity fills it with the verified `sub`.
    pub user_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub state: SubmissionState,
}

impl Submission {
    /// The suite is running.
    #[must_use]
    pub fn judging(&self) -> Self {
        Self {
            state: SubmissionState::Judging,
            ..self.clone()
        }
    }

    /// The suite finished with an outcome.
    #[must_use]
    pub fn completed(&self, outcome: SuiteOutcome, at: DateTime<Utc>) -> Self {
        Self {
            state: SubmissionState::Completed { outcome, at },
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubmissionState {
    /// Stored; the judging task hasn't picked it up yet.
    Pending,
    Judging,
    Completed {
        outcome: SuiteOutcome,
        at: DateTime<Utc>,
    },
}

impl SubmissionState {
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }
}

/// How a whole suite ended. The judging contract: run in AUTHORED ORDER, stop at the first
/// failure; `passed` is the count of consecutive passes from the top (the "8/118" semantics).
#[derive(Debug, Clone, PartialEq)]
pub enum SuiteOutcome {
    Accepted {
        total: usize,
    },
    Rejected {
        passed: usize,
        total: usize,
        first_failure: FailedCase,
    },
    /// The backend died mid-suite — machinery, NOT a verdict on the code.
    JudgeFailed {
        passed: usize,
        total: usize,
        detail: String,
    },
}

impl SuiteOutcome {
    pub fn passed_count(&self) -> usize {
        match self {
            Self::Accepted { total } => *total,
            Self::Rejected { passed, .. } | Self::JudgeFailed { passed, .. } => *passed,
        }
    }
}

/// The one revealed failure of a rejection.
#[derive(Debug, Clone, PartialEq)]
pub struct FailedCase {
    /// Zero-based position in the authored suite.
    pub index: usize,
    pub args: BTreeMap<String, String>,
    pub expected: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub status: RunStatus,
}
