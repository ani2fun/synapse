//! The source registry's Postgres adapter. Rows in, mount order out.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

use crate::catalog::application::{
    Audience, ContentReader, ContentSourceDraft, ContentSourceRecord, ContentSources, RegistryError,
    SyncOutcome, grouping_from_str, grouping_to_string,
};
use crate::identity::domain::Username;

const COLUMNS: &str =
    "id, repo, branch, grouping, sort_order, enabled, visibility, last_sha, last_synced_at, last_error";
const READER_COLUMNS: &str = "source_id, username, note, granted_at";

fn store_failed(error: &sqlx::Error) -> RegistryError {
    RegistryError::StoreFailed(error.to_string())
}

/// A row plus its readers. The list is loaded whole per `list` call — once a minute by the sync
/// loop, never per request — so two queries and a fold cost nothing worth a join.
fn record(
    row: &PgRow,
    readers: &BTreeMap<String, BTreeSet<Username>>,
) -> Result<ContentSourceRecord, RegistryError> {
    let id: String = row.get("id");
    let audience = Audience::parse(
        &row.get::<String, _>("visibility"),
        readers.get(&id).cloned().unwrap_or_default(),
    )?;
    Ok(ContentSourceRecord {
        repo: row.get("repo"),
        branch: row.get("branch"),
        grouping: grouping_from_str(&row.get::<String, _>("grouping")),
        order: row.get("sort_order"),
        enabled: row.get("enabled"),
        audience,
        last_sha: row.get("last_sha"),
        last_synced_at: row.get::<Option<DateTime<Utc>>, _>("last_synced_at"),
        last_error: row.get("last_error"),
        id,
    })
}

/// A stored name that no longer parses as a username — blank after a manual edit, say — is
/// dropped rather than admitted: a grant nobody can match is not a grant.
fn reader(row: &PgRow) -> Option<ContentReader> {
    Some(ContentReader {
        username: Username::parse(&row.get::<String, _>("username"))?,
        note: row.get("note"),
        granted_at: row.get::<DateTime<Utc>, _>("granted_at"),
    })
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
        let grants = sqlx::query("select source_id, username from content_source_reader")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        let mut readers: BTreeMap<String, BTreeSet<Username>> = BTreeMap::new();
        for grant in &grants {
            if let Some(name) = Username::parse(&grant.get::<String, _>("username")) {
                readers.entry(grant.get("source_id")).or_default().insert(name);
            }
        }
        rows.iter().map(|row| record(row, &readers)).collect()
    }

    /// Keyed on the derived id, so re-registering a repository edits its row rather than adding a
    /// second one. The sync columns are deliberately NOT reset: a placement change should not
    /// discard a good checkout and blank the book until the next tick.
    async fn upsert(&self, draft: &ContentSourceDraft) -> Result<ContentSourceRecord, RegistryError> {
        let id = draft.id();
        let row = sqlx::query(&format!(
            "insert into content_source (id, repo, branch, grouping, sort_order, enabled, visibility) \
             values ($1, $2, $3, $4, $5, $6, $7) \
             on conflict (id) do update set \
                repo = excluded.repo, branch = excluded.branch, grouping = excluded.grouping, \
                sort_order = excluded.sort_order, enabled = excluded.enabled, \
                visibility = excluded.visibility, updated_at = now() \
             returning {COLUMNS}"
        ))
        .bind(id)
        .bind(draft.repo())
        .bind(draft.branch())
        .bind(grouping_to_string(draft.grouping()))
        .bind(draft.order())
        .bind(draft.enabled())
        .bind(draft.visibility())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        tracing::info!(
            id = %id,
            repo = %draft.repo(),
            grouping = %grouping_to_string(draft.grouping()),
            enabled = draft.enabled(),
            visibility = draft.visibility(),
            "content source registered"
        );
        // The reader list survives a re-registration untouched, so it is read back rather than
        // assumed empty.
        let readers = self.readers_of(id).await?;
        record(&row, &BTreeMap::from([(id.to_owned(), readers)]))
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

    async fn list_readers(&self, id: &str) -> Result<Option<Vec<ContentReader>>, RegistryError> {
        if !self.exists(id).await? {
            return Ok(None);
        }
        let rows = sqlx::query(&format!(
            "select {READER_COLUMNS} from content_source_reader \
             where source_id = $1 order by granted_at desc, username"
        ))
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(Some(rows.iter().filter_map(reader).collect()))
    }

    async fn grant_reader(
        &self,
        id: &str,
        username: &Username,
        note: Option<&str>,
    ) -> Result<Option<ContentReader>, RegistryError> {
        if !self.exists(id).await? {
            return Ok(None);
        }
        let row = sqlx::query(&format!(
            "insert into content_source_reader (source_id, username, note) values ($1, $2, $3) \
             on conflict (source_id, username) do update set note = excluded.note \
             returning {READER_COLUMNS}"
        ))
        .bind(id)
        .bind(username.as_str())
        .bind(note)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        tracing::info!(source = id, reader = %username, "content source reader granted");
        Ok(reader(&row))
    }

    async fn revoke_reader(&self, id: &str, username: &Username) -> Result<bool, RegistryError> {
        let result = sqlx::query("delete from content_source_reader where source_id = $1 and username = $2")
            .bind(id)
            .bind(username.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        let revoked = result.rows_affected() > 0;
        if revoked {
            tracing::info!(source = id, reader = %username, "content source reader revoked");
        }
        Ok(revoked)
    }
}

impl PostgresContentSources {
    async fn exists(&self, id: &str) -> Result<bool, RegistryError> {
        let found = sqlx::query("select 1 from content_source where id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        Ok(found.is_some())
    }

    async fn readers_of(&self, id: &str) -> Result<BTreeSet<Username>, RegistryError> {
        let rows = sqlx::query("select username from content_source_reader where source_id = $1")
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        Ok(rows
            .iter()
            .filter_map(|row| Username::parse(&row.get::<String, _>("username")))
            .collect())
    }
}
