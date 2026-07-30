//! The Postgres progress adapter — the lightest kind of store (modelled on `PostgresLessonViews`):
//! a flat two-column key, upsert-on-mark, delete-on-reset. No ADT flattening, no JSONB.

use sqlx::PgPool;
use sqlx::Row;

use crate::progress::{ProblemProgressStore, ProgressError};

pub struct PostgresProblemProgress {
    pool: PgPool,
}

impl PostgresProblemProgress {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn store_failed(error: &sqlx::Error) -> ProgressError {
    ProgressError::StoreFailed(error.to_string())
}

impl ProblemProgressStore for PostgresProblemProgress {
    async fn mark(&self, user_id: &str, lesson_path: &str) -> Result<(), ProgressError> {
        // `do nothing` on the (user, lesson) PK keeps `mark` idempotent — a re-read or a repeat
        // accepted submission leaves the original `completed_at` in place.
        sqlx::query(
            "insert into problem_progress (user_id, lesson_path) values ($1, $2) \
             on conflict (user_id, lesson_path) do nothing",
        )
        .bind(user_id)
        .bind(lesson_path)
        .execute(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(())
    }

    async fn unmark(&self, user_id: &str, lesson_path: &str) -> Result<(), ProgressError> {
        // Deleting a row that is not there affects nothing and raises nothing, which is exactly
        // the idempotence `mark`'s `do nothing` gives the other direction.
        sqlx::query("delete from problem_progress where user_id = $1 and lesson_path = $2")
            .bind(user_id)
            .bind(lesson_path)
            .execute(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        Ok(())
    }

    async fn list_for(&self, user_id: &str) -> Result<Vec<String>, ProgressError> {
        let rows =
            sqlx::query("select lesson_path from problem_progress where user_id = $1 order by lesson_path")
                .bind(user_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| store_failed(&e))?;
        Ok(rows
            .iter()
            .map(|row| row.get::<String, _>("lesson_path"))
            .collect())
    }

    async fn reset_for(&self, user_id: &str) -> Result<usize, ProgressError> {
        let result = sqlx::query("delete from problem_progress where user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        Ok(usize::try_from(result.rows_affected()).unwrap_or(usize::MAX))
    }
}
