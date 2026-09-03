//! The Postgres canvas adapter. The body is the one jsonb column: it round-trips through
//! `serde_json` as the wire DTO itself, because that DTO IS the stored document — there is no
//! separate domain shape for it to be translated into and back out of.

use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use synapse_shared::canvas::{CanvasBodyDto, CanvasEntryDto};
use uuid::Uuid;

use crate::canvas::{CanvasError, CanvasStore};

pub struct PostgresCanvasStore {
    pool: PgPool,
}

impl PostgresCanvasStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn store_failed(error: &sqlx::Error) -> CanvasError {
    CanvasError::StoreFailed(error.to_string())
}

/// A stored row → the wire entry. The lesson path is stored `/`-joined (the shape `submissions`
/// uses) and splits back into segments here, so the client never has to know which side joined it.
fn to_entry(row: &PgRow) -> Result<CanvasEntryDto, CanvasError> {
    let id: Uuid = row.try_get("id").map_err(|e| store_failed(&e))?;
    let path: String = row.try_get("lesson_path").map_err(|e| store_failed(&e))?;
    let body: serde_json::Value = row.try_get("body").map_err(|e| store_failed(&e))?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(|e| store_failed(&e))?;
    // A body that will not decode is a row this build cannot serve; surfacing it as a store
    // failure keeps the caller honest rather than handing back a silently empty canvas.
    let body: CanvasBodyDto =
        serde_json::from_value(body).map_err(|e| CanvasError::StoreFailed(e.to_string()))?;
    Ok(CanvasEntryDto {
        id: id.to_string(),
        path: path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        body,
        created_at: created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
    })
}

impl CanvasStore for PostgresCanvasStore {
    async fn save(
        &self,
        user_id: &str,
        lesson_path: &str,
        body: &CanvasBodyDto,
    ) -> Result<CanvasEntryDto, CanvasError> {
        let encoded = serde_json::to_value(body).map_err(|e| CanvasError::StoreFailed(e.to_string()))?;
        // `returning *` rather than a second select: the id and `created_at` are the store's to
        // mint, and reading them back in the same statement is what makes the reply authoritative.
        let row = sqlx::query(
            "insert into canvas_entries (id, user_id, lesson_path, body) \
             values ($1, $2, $3, $4) returning *",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(lesson_path)
        .bind(encoded)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        to_entry(&row)
    }

    async fn list_for(&self, user_id: &str, lesson_path: &str) -> Result<Vec<CanvasEntryDto>, CanvasError> {
        let rows = sqlx::query(
            "select * from canvas_entries where user_id = $1 and lesson_path = $2 \
             order by created_at desc",
        )
        .bind(user_id)
        .bind(lesson_path)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        rows.iter().map(to_entry).collect()
    }

    async fn delete(&self, user_id: &str, id: &str) -> Result<(), CanvasError> {
        let Ok(uuid) = id.parse::<Uuid>() else {
            return Err(CanvasError::NotFound(id.to_owned()));
        };
        // Read the owner first so "no such entry" and "not yours" stay distinguishable. A single
        // `delete ... where id = $1 and user_id = $2` collapses both into 0 rows affected, and the
        // edge would have to answer one of them wrongly.
        let owner: Option<String> = sqlx::query("select user_id from canvas_entries where id = $1")
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?
            .map(|row| row.get::<String, _>("user_id"));
        match owner {
            None => Err(CanvasError::NotFound(id.to_owned())),
            Some(owner) if owner != user_id => Err(CanvasError::NotYours(id.to_owned())),
            Some(_) => {
                sqlx::query("delete from canvas_entries where id = $1")
                    .bind(uuid)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| store_failed(&e))?;
                Ok(())
            }
        }
    }

    async fn erase_all_for(&self, user_id: &str) -> Result<usize, CanvasError> {
        let result = sqlx::query("delete from canvas_entries where user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        Ok(usize::try_from(result.rows_affected()).unwrap_or(usize::MAX))
    }
}
