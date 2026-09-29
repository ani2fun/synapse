//! The Postgres note adapter. One row per (user, problem); a save is an upsert and an empty save
//! is a delete, so the table only ever holds notes that say something.

use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use synapse_shared::notes::NoteDto;

use crate::notes::{NoteStore, NotesError};

pub struct PostgresNoteStore {
    pool: PgPool,
}

impl PostgresNoteStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn store_failed(error: &sqlx::Error) -> NotesError {
    NotesError::StoreFailed(error.to_string())
}

/// A stored row → the wire note. The path is stored `/`-joined and splits back into segments
/// here, the way `canvas_entries` does it.
fn to_note(row: &PgRow) -> Result<NoteDto, NotesError> {
    let path: String = row.try_get("lesson_path").map_err(|e| store_failed(&e))?;
    let text: String = row.try_get("body").map_err(|e| store_failed(&e))?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(|e| store_failed(&e))?;
    Ok(NoteDto {
        path: path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        text,
        updated_at: Some(updated_at.to_rfc3339_opts(SecondsFormat::Millis, true)),
    })
}

impl NoteStore for PostgresNoteStore {
    async fn get(&self, user_id: &str, lesson_path: &str) -> Result<Option<NoteDto>, NotesError> {
        let row = sqlx::query("select * from problem_notes where user_id = $1 and lesson_path = $2")
            .bind(user_id)
            .bind(lesson_path)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        row.as_ref().map(to_note).transpose()
    }

    async fn put(&self, user_id: &str, lesson_path: &str, text: &str) -> Result<Option<NoteDto>, NotesError> {
        if text.is_empty() {
            sqlx::query("delete from problem_notes where user_id = $1 and lesson_path = $2")
                .bind(user_id)
                .bind(lesson_path)
                .execute(&self.pool)
                .await
                .map_err(|e| store_failed(&e))?;
            return Ok(None);
        }
        // `returning *` so `updated_at` in the reply is the one actually stored — the client
        // records it as the version its local copy is based on.
        let row = sqlx::query(
            "insert into problem_notes (user_id, lesson_path, body) values ($1, $2, $3) \
             on conflict (user_id, lesson_path) \
             do update set body = excluded.body, updated_at = now() \
             returning *",
        )
        .bind(user_id)
        .bind(lesson_path)
        .bind(text)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        to_note(&row).map(Some)
    }

    async fn erase_all_for(&self, user_id: &str) -> Result<usize, NotesError> {
        let result = sqlx::query("delete from problem_notes where user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        Ok(usize::try_from(result.rows_affected()).unwrap_or(usize::MAX))
    }
}
