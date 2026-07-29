//! The source registry's Postgres adapter. Rows in, mount order out.

use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

use crate::catalog::application::{
    ContentSourceDraft, ContentSourceRecord, ContentSources, RegistryError, SyncOutcome, grouping_from_str,
    grouping_to_string,
};

const COLUMNS: &str = "id, repo, branch, grouping, sort_order, enabled, last_sha, last_synced_at, last_error";

fn store_failed(error: &sqlx::Error) -> RegistryError {
    RegistryError::StoreFailed(error.to_string())
}

fn record(row: &PgRow) -> ContentSourceRecord {
    ContentSourceRecord {
        id: row.get("id"),
        repo: row.get("repo"),
        branch: row.get("branch"),
        grouping: grouping_from_str(&row.get::<String, _>("grouping")),
        order: row.get("sort_order"),
        enabled: row.get("enabled"),
        last_sha: row.get("last_sha"),
        last_synced_at: row.get::<Option<DateTime<Utc>>, _>("last_synced_at"),
        last_error: row.get("last_error"),
    }
}

pub struct PostgresContentSources {
    pool: PgPool,
}

impl PostgresContentSources {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ContentSources for PostgresContentSources {
    /// Mount order: enabled first, then the configured order, then the id so the sequence is
    /// stable across restarts. Stability matters — the merge's first-wins rule is decided by it.
    async fn list(&self) -> Result<Vec<ContentSourceRecord>, RegistryError> {
        let rows = sqlx::query(&format!(
            "select {COLUMNS} from content_source \
             order by enabled desc, sort_order nulls last, id"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(rows.iter().map(record).collect())
    }

    /// Keyed on the derived id, so re-registering a repository edits its row rather than adding a
    /// second one. The sync columns are deliberately NOT reset: a placement change should not
    /// discard a good checkout and blank the book until the next tick.
    async fn upsert(&self, draft: &ContentSourceDraft) -> Result<ContentSourceRecord, RegistryError> {
        let id = draft.id();
        let row = sqlx::query(&format!(
            "insert into content_source (id, repo, branch, grouping, sort_order, enabled) \
             values ($1, $2, $3, $4, $5, $6) \
             on conflict (id) do update set \
                repo = excluded.repo, branch = excluded.branch, grouping = excluded.grouping, \
                sort_order = excluded.sort_order, enabled = excluded.enabled, updated_at = now() \
             returning {COLUMNS}"
        ))
        .bind(id)
        .bind(draft.repo())
        .bind(draft.branch())
        .bind(grouping_to_string(draft.grouping()))
        .bind(draft.order())
        .bind(draft.enabled())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        tracing::info!(
            id = %id,
            repo = %draft.repo(),
            grouping = %grouping_to_string(draft.grouping()),
            enabled = draft.enabled(),
            "content source registered"
        );
        Ok(record(&row))
    }

    async fn remove(&self, id: &str) -> Result<bool, RegistryError> {
        let result = sqlx::query("delete from content_source where id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        let removed = result.rows_affected() > 0;
        if removed {
            tracing::info!(id, "content source removed");
        }
        Ok(removed)
    }

    /// A failure keeps the last good sha on purpose: a broken push degrades the book to stale,
    /// not to absent, and the error rides alongside so the admin panel can say which it is.
    async fn record_sync(&self, id: &str, outcome: &SyncOutcome) -> Result<(), RegistryError> {
        let query = match outcome {
            SyncOutcome::Landed(sha) => sqlx::query(
                "update content_source \
                 set last_sha = $2, last_synced_at = now(), last_error = null, updated_at = now() \
                 where id = $1",
            )
            .bind(id)
            .bind(sha),
            SyncOutcome::Failed(detail) => sqlx::query(
                "update content_source \
                 set last_error = $2, last_synced_at = now(), updated_at = now() where id = $1",
            )
            .bind(id)
            .bind(detail),
        };
        query.execute(&self.pool).await.map_err(|e| store_failed(&e))?;
        Ok(())
    }
}
