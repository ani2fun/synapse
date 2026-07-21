//! The Postgres `SubmissionRepository`. The state ADT flattens to
//! `(status, outcome jsonb, completed_at)` at this edge only. The JSONB codecs are
//! ADAPTER-OWNED (storage format ≠ wire contract) and use a fixed externally-tagged wrapper
//! object shape (`{"Rejected":{"passed":8,…,"firstFailure":{…}}}`) so rows written by an
//! earlier deployment of this service remain readable.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use synapse_shared::execution::RunStatus;

use crate::submission::application::{SubmissionError, SubmissionRepository};
use crate::submission::domain::{FailedCase, Submission, SubmissionId, SubmissionState, SuiteOutcome};

pub struct PostgresSubmissionRepository {
    pool: PgPool,
}

impl PostgresSubmissionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn store_failed(error: impl std::fmt::Display) -> SubmissionError {
    SubmissionError::StoreFailed(error.to_string())
}

impl SubmissionRepository for PostgresSubmissionRepository {
    async fn save(&self, s: &Submission) -> Result<(), SubmissionError> {
        let (status, outcome, completed_at) = flatten(&s.state);
        sqlx::query(
            "insert into submissions \
             (id, lesson_path, language, source, user_id, created_at, status, outcome, completed_at) \
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(s.id.0)
        .bind(s.lesson_path.join("/"))
        .bind(&s.language)
        .bind(&s.source)
        .bind(&s.user_id)
        .bind(s.created_at)
        .bind(status)
        .bind(outcome)
        .bind(completed_at)
        .execute(&self.pool)
        .await
        .map_err(store_failed)?;
        Ok(())
    }

    async fn update(&self, s: &Submission) -> Result<(), SubmissionError> {
        let (status, outcome, completed_at) = flatten(&s.state);
        sqlx::query("update submissions set status = $1, outcome = $2, completed_at = $3 where id = $4")
            .bind(status)
            .bind(outcome)
            .bind(completed_at)
            .bind(s.id.0)
            .execute(&self.pool)
            .await
            .map_err(store_failed)?;
        Ok(())
    }

    async fn get(&self, id: SubmissionId) -> Result<Option<Submission>, SubmissionError> {
        let row = sqlx::query("select * from submissions where id = $1")
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(store_failed)?;
        row.map(|r| read(&r)).transpose()
    }

    async fn list_for(
        &self,
        lesson_path: &[String],
        by_user: Option<&str>,
    ) -> Result<Vec<Submission>, SubmissionError> {
        let joined = lesson_path.join("/");
        let rows = match by_user {
            Some(user) => {
                sqlx::query(
                    "select * from submissions where lesson_path = $1 and user_id = $2 \
                     order by created_at desc",
                )
                .bind(joined)
                .bind(user)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query("select * from submissions where lesson_path = $1 order by created_at desc")
                    .bind(joined)
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(store_failed)?;
        rows.iter().map(read).collect()
    }

    async fn unfinished_before(&self, cutoff: DateTime<Utc>) -> Result<Vec<Submission>, SubmissionError> {
        let rows = sqlx::query(
            "select * from submissions where status in ('pending', 'judging') and created_at < $1 \
             order by created_at",
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await
        .map_err(store_failed)?;
        rows.iter().map(read).collect()
    }

    async fn delete(&self, id: SubmissionId) -> Result<(), SubmissionError> {
        sqlx::query("delete from submissions where id = $1")
            .bind(id.0)
            .execute(&self.pool)
            .await
            .map_err(store_failed)?;
        Ok(())
    }

    async fn delete_all_for(&self, user_id: &str) -> Result<usize, SubmissionError> {
        let result = sqlx::query("delete from submissions where user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(store_failed)?;
        Ok(usize::try_from(result.rows_affected()).unwrap_or(usize::MAX))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FLATTEN / READ — the ADT ↔ row edge
// ─────────────────────────────────────────────────────────────────────────────

fn flatten(state: &SubmissionState) -> (&'static str, Option<serde_json::Value>, Option<DateTime<Utc>>) {
    match state {
        SubmissionState::Pending => ("pending", None, None),
        SubmissionState::Judging => ("judging", None, None),
        SubmissionState::Completed { outcome, at } => (
            "completed",
            serde_json::to_value(OutcomeJson::from(outcome)).ok(),
            Some(*at),
        ),
    }
}

fn read(row: &PgRow) -> Result<Submission, SubmissionError> {
    let status: String = row.try_get("status").map_err(store_failed)?;
    let state = match status.as_str() {
        "pending" => SubmissionState::Pending,
        "judging" => SubmissionState::Judging,
        "completed" => {
            let outcome: serde_json::Value = row.try_get("outcome").map_err(store_failed)?;
            let decoded: OutcomeJson = serde_json::from_value(outcome).map_err(store_failed)?;
            let at: DateTime<Utc> = row.try_get("completed_at").map_err(store_failed)?;
            SubmissionState::Completed {
                outcome: decoded.into(),
                at,
            }
        }
        other => return Err(SubmissionError::StoreFailed(format!("unknown status '{other}'"))),
    };
    let lesson_path: String = row.try_get("lesson_path").map_err(store_failed)?;
    Ok(Submission {
        id: SubmissionId(row.try_get("id").map_err(store_failed)?),
        lesson_path: lesson_path.split('/').map(str::to_owned).collect(),
        language: row.try_get("language").map_err(store_failed)?,
        source: row.try_get("source").map_err(store_failed)?,
        user_id: row.try_get("user_id").map_err(store_failed)?,
        created_at: row.try_get("created_at").map_err(store_failed)?,
        state,
    })
}

/// The adapter-owned JSONB shape — circe-derived parity (externally tagged, camelCase fields,
/// `RunStatus` as its case name).
#[derive(Serialize, Deserialize)]
enum OutcomeJson {
    Accepted {
        total: usize,
    },
    Rejected {
        passed: usize,
        total: usize,
        #[serde(rename = "firstFailure")]
        first_failure: FailedCaseJson,
    },
    JudgeFailed {
        passed: usize,
        total: usize,
        detail: String,
    },
}

#[derive(Serialize, Deserialize)]
struct FailedCaseJson {
    index: usize,
    args: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expected: Option<String>,
    stdout: String,
    stderr: String,
    status: RunStatus,
}

impl From<&SuiteOutcome> for OutcomeJson {
    fn from(outcome: &SuiteOutcome) -> Self {
        match outcome {
            SuiteOutcome::Accepted { total } => Self::Accepted { total: *total },
            SuiteOutcome::Rejected {
                passed,
                total,
                first_failure,
            } => Self::Rejected {
                passed: *passed,
                total: *total,
                first_failure: FailedCaseJson {
                    index: first_failure.index,
                    args: first_failure.args.clone(),
                    expected: first_failure.expected.clone(),
                    stdout: first_failure.stdout.clone(),
                    stderr: first_failure.stderr.clone(),
                    status: first_failure.status,
                },
            },
            SuiteOutcome::JudgeFailed {
                passed,
                total,
                detail,
            } => Self::JudgeFailed {
                passed: *passed,
                total: *total,
                detail: detail.clone(),
            },
        }
    }
}

impl From<OutcomeJson> for SuiteOutcome {
    fn from(json: OutcomeJson) -> Self {
        match json {
            OutcomeJson::Accepted { total } => Self::Accepted { total },
            OutcomeJson::Rejected {
                passed,
                total,
                first_failure,
            } => Self::Rejected {
                passed,
                total,
                first_failure: FailedCase {
                    index: first_failure.index,
                    args: first_failure.args,
                    expected: first_failure.expected,
                    stdout: first_failure.stdout,
                    stderr: first_failure.stderr,
                    status: first_failure.status,
                },
            },
            OutcomeJson::JudgeFailed {
                passed,
                total,
                detail,
            } => Self::JudgeFailed {
                passed,
                total,
                detail,
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_jsonb_shape_is_circe_parity() {
        let outcome = SuiteOutcome::Rejected {
            passed: 8,
            total: 118,
            first_failure: FailedCase {
                index: 8,
                args: BTreeMap::from([("n".to_owned(), "5".to_owned())]),
                expected: Some("120".to_owned()),
                stdout: "119\n".to_owned(),
                stderr: String::new(),
                status: RunStatus::Accepted,
            },
        };
        let json = serde_json::to_value(OutcomeJson::from(&outcome)).unwrap();
        // The externally-tagged wrapper object + camelCase + case-name status — byte-compatible
        // with rows written by an earlier deployment of this service.
        assert_eq!(json["Rejected"]["passed"], 8);
        assert_eq!(json["Rejected"]["firstFailure"]["status"], "Accepted");
        assert_eq!(json["Rejected"]["firstFailure"]["expected"], "120");
        let back: SuiteOutcome = serde_json::from_value::<OutcomeJson>(json).unwrap().into();
        assert_eq!(back, outcome);
    }
}
