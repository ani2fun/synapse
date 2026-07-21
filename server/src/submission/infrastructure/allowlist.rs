//! The Postgres allowlist adapter (`PostgresSubmissionAllowlist`) — the probe the
//! submit gate rides plus the admin panel's management verbs.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::Row;
use sqlx::postgres::PgRow;

use crate::submission::application::{AllowlistEntry, SubmissionAllowlist, SubmissionError};

pub struct PostgresSubmissionAllowlist {
    pool: PgPool,
}

impl PostgresSubmissionAllowlist {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn store_failed(error: &sqlx::Error) -> SubmissionError {
    SubmissionError::StoreFailed(error.to_string())
}

fn entry(row: &PgRow) -> AllowlistEntry {
    AllowlistEntry {
        username: row.get("username"),
        note: row.get("note"),
        granted_at: row.get::<DateTime<Utc>, _>("granted_at"),
    }
}

impl SubmissionAllowlist for PostgresSubmissionAllowlist {
    async fn is_allowed(&self, username: &str) -> Result<bool, SubmissionError> {
        let row = sqlx::query("select 1 as one from submission_allowlist where username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        let allowed = row.is_some_and(|r| r.get::<i32, _>("one") == 1);
        tracing::debug!(username, allowed, "submission allowlist checked");
        Ok(allowed)
    }

    async fn list(&self) -> Result<Vec<AllowlistEntry>, SubmissionError> {
        let rows = sqlx::query(
            "select username, note, granted_at from submission_allowlist \
             order by granted_at desc, username",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(rows.iter().map(entry).collect())
    }

    async fn grant(&self, username: &str, note: Option<&str>) -> Result<AllowlistEntry, SubmissionError> {
        let row = sqlx::query(
            "insert into submission_allowlist (username, note) values ($1, $2) \
             on conflict (username) do update set note = excluded.note \
             returning username, note, granted_at",
        )
        .bind(username)
        .bind(note)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        tracing::info!(username, "submission allowlist: granted");
        Ok(entry(&row))
    }

    async fn revoke(&self, username: &str) -> Result<bool, SubmissionError> {
        let result = sqlx::query("delete from submission_allowlist where username = $1")
            .bind(username)
            .execute(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        let revoked = result.rows_affected() > 0;
        if revoked {
            tracing::info!(username, "submission allowlist: revoked");
        }
        Ok(revoked)
    }
}
