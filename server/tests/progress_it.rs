//! Per-user progress ITs. The store half is gated Postgres (`POSTGRES_IT=1`, db on :5532, the
//! `postgres_it.rs` convention): the `PostgresProblemProgress` adapter's SQL, and the load-bearing
//! guarantee that resetting progress leaves the caller's SUBMISSIONS untouched. The HTTP half is
//! ungated — the anonymous policy short-circuits before any store touch (lazy pool never connects).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use synapse_server::progress::{PostgresProblemProgress, ProblemProgressStore};
use synapse_server::submission::application::SubmissionRepository;
use synapse_server::submission::domain::{Submission, SubmissionId, SubmissionState};
use synapse_server::submission::infrastructure::PostgresSubmissionRepository;
use tower::ServiceExt;
use uuid::Uuid;

const IT_PREFIX: &str = "it-rs-progress";

/// A gated pool with THIS test's `problem_progress` rows cleared. Each test owns a distinct
/// `user_id` namespace, so the suite is safe under default parallelism (the `postgres_it` lesson).
async fn progress_pool(scope: &str) -> Option<(PgPool, String)> {
    let pool = gated_pool().await?;
    let user = format!("{IT_PREFIX}-{scope}");
    sqlx::query("delete from problem_progress where user_id = $1")
        .bind(&user)
        .execute(&pool)
        .await
        .unwrap();
    Some((pool, user))
}

async fn gated_pool() -> Option<PgPool> {
    if std::env::var("POSTGRES_IT").is_err() {
        eprintln!("skipped (set POSTGRES_IT=1 with docker compose db on :5532)");
        return None;
    }
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://synapse:synapse@localhost:5532/synapse_rs".to_owned());
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .unwrap();
    sqlx::migrate!("../migrations").run(&pool).await.unwrap();
    Some(pool)
}

#[tokio::test]
async fn progress_marks_idempotently_lists_ordered_and_resets() {
    let Some((pool, user)) = progress_pool("roundtrip").await else {
        return;
    };
    let store = PostgresProblemProgress::new(pool);

    store.mark(&user, "dsa/two-sum").await.unwrap();
    store.mark(&user, "dsa/two-sum").await.unwrap(); // idempotent — the PK conflict is a no-op
    store.mark(&user, "dsa/reverse").await.unwrap();

    let listed = store.list_for(&user).await.unwrap();
    assert_eq!(
        listed,
        vec!["dsa/reverse".to_owned(), "dsa/two-sum".to_owned()],
        "ordered by path, one row per lesson"
    );

    let removed = store.reset_for(&user).await.unwrap();
    assert_eq!(removed, 2, "reset returns the row count cleared");
    assert!(
        store.list_for(&user).await.unwrap().is_empty(),
        "nothing left after a reset"
    );
}

/// The load-bearing guarantee: "reset progress" is NOT "erase my data" — it clears the progress
/// rows and leaves the caller's submission history in place.
#[tokio::test]
async fn resetting_progress_leaves_submissions_intact() {
    let Some((pool, user)) = progress_pool("keeps-subs").await else {
        return;
    };
    sqlx::query("delete from submissions where user_id = $1")
        .bind(&user)
        .execute(&pool)
        .await
        .unwrap();
    let progress = PostgresProblemProgress::new(pool.clone());
    let submissions = PostgresSubmissionRepository::new(pool.clone());

    let sub = Submission {
        id: SubmissionId(Uuid::new_v4()),
        lesson_path: vec!["dsa".to_owned(), "solved".to_owned()],
        language: "python".to_owned(),
        source: "print(1)".to_owned(),
        user_id: Some(user.clone()),
        created_at: Utc::now(),
        state: SubmissionState::Pending,
    };
    submissions.save(&sub).await.unwrap();
    progress.mark(&user, "dsa/solved").await.unwrap();

    progress.reset_for(&user).await.unwrap();

    assert!(
        progress.list_for(&user).await.unwrap().is_empty(),
        "progress is cleared"
    );
    assert!(
        submissions.get(sub.id).await.unwrap().is_some(),
        "the submission survives a progress reset"
    );

    sqlx::query("delete from submissions where user_id = $1")
        .bind(&user)
        .execute(&pool)
        .await
        .unwrap();
}

/// Anonymous callers: GET is `[]` (store untouched), writes 401 — the never-silently-anonymous
/// policy. No token minting needed: every anonymous path short-circuits before the store, so the
/// lazy pool is never dialed and this runs without `POSTGRES_IT`.
#[tokio::test]
async fn anonymous_progress_lists_empty_and_cannot_write() {
    let app = common::app_with(Path::new("__no_content__"), "http://127.0.0.1:9", None);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 4096).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["completed"], json!([]), "anonymous sees an empty list");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/progress")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "path": "dsa/x" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "anonymous cannot mark");

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "anonymous cannot reset");
}
