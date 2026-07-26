//! The Postgres adapters: the content-editor allowlist (the propose gate's probe plus the admin
//! panel's verbs) and the edit-request store (the reuse rule's memory).

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::Row;
use sqlx::postgres::PgRow;
use uuid::Uuid;

use crate::authoring::application::{
    AuthoringError, ContentEditorEntry, ContentEditors, EditRequestRepository,
};
use crate::authoring::domain::{EditRequest, EditRequestId, EditRequestState, PullRequestRef};

fn store_failed(error: &sqlx::Error) -> AuthoringError {
    AuthoringError::StoreFailed(error.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// THE ALLOWLIST
// ─────────────────────────────────────────────────────────────────────────────

pub struct PostgresContentEditors {
    pool: PgPool,
}

impl PostgresContentEditors {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn editor(row: &PgRow) -> ContentEditorEntry {
    ContentEditorEntry {
        username: row.get("username"),
        note: row.get("note"),
        granted_at: row.get::<DateTime<Utc>, _>("granted_at"),
    }
}

impl ContentEditors for PostgresContentEditors {
    async fn is_allowed(&self, username: &str) -> Result<bool, AuthoringError> {
        let row = sqlx::query("select 1 as one from content_editor_allowlist where username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        let allowed = row.is_some_and(|r| r.get::<i32, _>("one") == 1);
        tracing::debug!(username, allowed, "content-editor allowlist checked");
        Ok(allowed)
    }

    async fn list(&self) -> Result<Vec<ContentEditorEntry>, AuthoringError> {
        let rows = sqlx::query(
            "select username, note, granted_at from content_editor_allowlist \
             order by granted_at desc, username",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(rows.iter().map(editor).collect())
    }

    async fn grant(&self, username: &str, note: Option<&str>) -> Result<ContentEditorEntry, AuthoringError> {
        let row = sqlx::query(
            "insert into content_editor_allowlist (username, note) values ($1, $2) \
             on conflict (username) do update set note = excluded.note \
             returning username, note, granted_at",
        )
        .bind(username)
        .bind(note)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        tracing::info!(username, "content-editor allowlist: granted");
        Ok(editor(&row))
    }

    async fn revoke(&self, username: &str) -> Result<bool, AuthoringError> {
        let result = sqlx::query("delete from content_editor_allowlist where username = $1")
            .bind(username)
            .execute(&self.pool)
            .await
            .map_err(|e| store_failed(&e))?;
        let revoked = result.rows_affected() > 0;
        if revoked {
            tracing::info!(username, "content-editor allowlist: revoked");
        }
        Ok(revoked)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE EDIT-REQUEST STORE
// ─────────────────────────────────────────────────────────────────────────────

pub struct PostgresEditRequests {
    pool: PgPool,
}

impl PostgresEditRequests {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const COLUMNS: &str = "id, username, lesson_path, file_path, repo, branch, attempt, \
                       pr_number, pr_url, state, commits, created_at, updated_at";

/// `pr_number`/`pr_url` are nullable together — a dry-run row has a branch and no pull request —
/// so a half-populated pair reads as no pull request rather than a panic.
fn request(row: &PgRow) -> EditRequest {
    let pull_request = match (
        row.get::<Option<i64>, _>("pr_number"),
        row.get::<Option<String>, _>("pr_url"),
    ) {
        (Some(number), Some(url)) => Some(PullRequestRef {
            number: number.unsigned_abs(),
            url,
        }),
        _ => None,
    };
    EditRequest {
        id: EditRequestId(row.get::<Uuid, _>("id")),
        username: row.get("username"),
        lesson_path: row.get("lesson_path"),
        file_path: row.get("file_path"),
        repo: row.get("repo"),
        branch: row.get("branch"),
        attempt: row.get::<i32, _>("attempt").unsigned_abs(),
        pull_request,
        state: EditRequestState::parse(&row.get::<String, _>("state")),
        commits: row.get::<i32, _>("commits").unsigned_abs(),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    }
}

impl EditRequestRepository for PostgresEditRequests {
    async fn open_for(
        &self,
        username: &str,
        lesson_path: &str,
    ) -> Result<Option<EditRequest>, AuthoringError> {
        // Newest first: there should only ever be one open row per (user, page), but if a
        // half-finished write ever left two, reusing the LATEST is the safe reading.
        let row = sqlx::query(&format!(
            "select {COLUMNS} from content_edit_request \
             where username = $1 and lesson_path = $2 and state = 'open' \
             order by attempt desc limit 1"
        ))
        .bind(username)
        .bind(lesson_path)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(row.as_ref().map(request))
    }

    async fn highest_attempt(&self, username: &str, lesson_path: &str) -> Result<u32, AuthoringError> {
        let row = sqlx::query(
            "select coalesce(max(attempt), 0) as top from content_edit_request \
             where username = $1 and lesson_path = $2",
        )
        .bind(username)
        .bind(lesson_path)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(row.get::<i32, _>("top").unsigned_abs())
    }

    async fn save(&self, request: &EditRequest) -> Result<(), AuthoringError> {
        sqlx::query(
            "insert into content_edit_request \
             (id, username, lesson_path, file_path, repo, branch, attempt, pr_number, pr_url, \
              state, commits, created_at, updated_at) \
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(request.id.0)
        .bind(&request.username)
        .bind(&request.lesson_path)
        .bind(&request.file_path)
        .bind(&request.repo)
        .bind(&request.branch)
        .bind(i32::try_from(request.attempt).unwrap_or(i32::MAX))
        .bind(
            request
                .pull_request
                .as_ref()
                .map(|pr| i64::try_from(pr.number).unwrap_or(i64::MAX)),
        )
        .bind(request.pull_request.as_ref().map(|pr| pr.url.clone()))
        .bind(request.state.as_str())
        .bind(i32::try_from(request.commits).unwrap_or(i32::MAX))
        .bind(request.created_at)
        .bind(request.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(())
    }

    /// Everything mutable travels together — the branch and the attempt are the row's identity and
    /// never change.
    async fn update(&self, request: &EditRequest) -> Result<(), AuthoringError> {
        sqlx::query(
            "update content_edit_request \
             set pr_number = $2, pr_url = $3, state = $4, commits = $5, updated_at = $6 \
             where id = $1",
        )
        .bind(request.id.0)
        .bind(
            request
                .pull_request
                .as_ref()
                .map(|pr| i64::try_from(pr.number).unwrap_or(i64::MAX)),
        )
        .bind(request.pull_request.as_ref().map(|pr| pr.url.clone()))
        .bind(request.state.as_str())
        .bind(i32::try_from(request.commits).unwrap_or(i32::MAX))
        .bind(request.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(())
    }

    async fn list_for(&self, username: &str) -> Result<Vec<EditRequest>, AuthoringError> {
        let rows = sqlx::query(&format!(
            "select {COLUMNS} from content_edit_request where username = $1 \
             order by updated_at desc, created_at desc"
        ))
        .bind(username)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_failed(&e))?;
        Ok(rows.iter().map(request).collect())
    }
}
