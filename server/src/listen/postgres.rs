//! The Postgres listen allowlist — the probe the listen gate rides plus the admin panel's
//! management verbs. The same three columns as its two siblings, and deliberately a third table:
//! each grant is revoked on its own.

use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use synapse_shared::submission::AllowlistEntryDto;

use crate::identity::domain::Username;
use crate::listen::ListenError;

pub struct PostgresListenAllowlist {
    pool: PgPool,
}

fn store_failed(error: &sqlx::Error) -> ListenError {
    ListenError::StoreFailed(error.to_string())
}

/// Straight to the shared wire shape: the admin page renders all three lists with one component.
fn entry(row: &PgRow) -> AllowlistEntryDto {
    AllowlistEntryDto {
        username: row.get("username"),
        note: row.get("note"),
        granted_at: row
            .get::<DateTime<Utc>, _>("granted_at")
            .to_rfc3339_opts(SecondsFormat::Millis, true),
    }
}

impl PostgresListenAllowlist {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn is_allowed(&self, username: &Username) -> Result<bool, ListenError> {
        let row = sqlx::query("select 1 as one from listen_allowlist where username = $1")
            .bind(username.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        let allowed = row.is_some();
        tracing::debug!(%username, allowed, "listen allowlist checked");
        Ok(allowed)
    }

    pub async fn list(&self) -> Result<Vec<AllowlistEntryDto>, ListenError> {
        let rows = sqlx::query(
            "select username, note, granted_at from listen_allowlist order by granted_at desc, username",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(rows.iter().map(entry).collect())
    }

    pub async fn grant(
        &self,
        username: &Username,
        note: Option<&str>,
    ) -> Result<AllowlistEntryDto, ListenError> {
        let row = sqlx::query(
            "insert into listen_allowlist (username, note) values ($1, $2) \
             on conflict (username) do update set note = excluded.note \
             returning username, note, granted_at",
        )
        .bind(username.as_str())
        .bind(note)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        tracing::info!(%username, "listen allowlist: granted");
        Ok(entry(&row))
    }

    pub async fn revoke(&self, username: &Username) -> Result<bool, ListenError> {
        let result = sqlx::query("delete from listen_allowlist where username = $1")
            .bind(username.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        let revoked = result.rows_affected() > 0;
        if revoked {
            tracing::info!(%username, "listen allowlist: revoked");
        }
        Ok(revoked)
    }
}
