//! Problem-notes ITs. The store half is gated Postgres (`POSTGRES_IT=1`, db on :5532, the
//! `postgres_it.rs` convention): the upsert, the empty-save delete and the per-account isolation.
//! The HTTP half is ungated — anonymous paths and the length check both answer before a store
//! touch, so the lazy pool is never dialed.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use synapse_server::notes::{NoteStore, PostgresNoteStore};
use synapse_shared::notes::NOTE_MAX_UTF16;
use tower::ServiceExt;

const IT_PREFIX: &str = "it-rs-notes";

/// A gated pool with THIS test's rows cleared. Each test owns a distinct `user_id` namespace, so
/// the suite is safe under default parallelism.
async fn notes_pool(scope: &str) -> Option<(PgPool, String)> {
    let pool = common::gated_pool().await?;
    let user = format!("{IT_PREFIX}-{scope}");
    sqlx::query("delete from problem_notes where user_id like $1")
        .bind(format!("{user}%"))
        .execute(&pool)
        .await
        .unwrap();
    Some((pool, user))
}

/// A save REPLACES the note — one row per problem, never a growing history — and the reply
/// carries the stored timestamp, which the client records as the version it is based on.
#[tokio::test]
async fn a_save_replaces_the_note_and_bumps_its_timestamp() {
    let Some((pool, user)) = notes_pool("upsert").await else {
        return;
    };
    let store = PostgresNoteStore::new(pool.clone());

    assert_eq!(
        store.get(&user, "dsa/two-sum").await.unwrap(),
        None,
        "nothing yet"
    );

    let first = store
        .put(&user, "dsa/two-sum", "## first")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.text, "## first");
    assert_eq!(first.path, vec!["dsa", "two-sum"]);
    assert!(first.updated_at.is_some());

    let second = store
        .put(&user, "dsa/two-sum", "## second")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        store.get(&user, "dsa/two-sum").await.unwrap(),
        Some(second.clone())
    );
    assert!(
        second.updated_at >= first.updated_at,
        "RFC 3339 in UTC orders lexically"
    );

    let rows: i64 = sqlx::query_scalar("select count(*) from problem_notes where user_id = $1")
        .bind(&user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1, "an upsert, not an append");
}

/// Saving an empty note deletes it: a problem opened and never written in leaves nothing behind.
#[tokio::test]
async fn an_empty_save_deletes_the_note() {
    let Some((pool, user)) = notes_pool("empty").await else {
        return;
    };
    let store = PostgresNoteStore::new(pool);

    store.put(&user, "dsa/two-sum", "something").await.unwrap();
    assert_eq!(store.put(&user, "dsa/two-sum", "").await.unwrap(), None);
    assert_eq!(store.get(&user, "dsa/two-sum").await.unwrap(), None);
    // And an empty save of a note that never existed is not an error.
    assert_eq!(store.put(&user, "dsa/never", "").await.unwrap(), None);
}

/// One account's notes are invisible to — and survive the erase of — another's, and erase
/// clears every problem at once.
#[tokio::test]
async fn notes_are_per_account_and_erase_clears_only_the_callers() {
    let Some((pool, user)) = notes_pool("owner").await else {
        return;
    };
    let other = format!("{user}-other");
    let store = PostgresNoteStore::new(pool);

    store.put(&user, "dsa/two-sum", "mine").await.unwrap();
    store.put(&user, "dsa/three-sum", "mine too").await.unwrap();
    store.put(&other, "dsa/two-sum", "theirs").await.unwrap();

    assert_eq!(
        store.get(&other, "dsa/two-sum").await.unwrap().unwrap().text,
        "theirs"
    );
    assert_eq!(store.erase_all_for(&user).await.unwrap(), 2);
    assert_eq!(store.get(&user, "dsa/two-sum").await.unwrap(), None);
    assert_eq!(
        store.get(&other, "dsa/two-sum").await.unwrap().unwrap().text,
        "theirs",
        "another account's notes are untouched"
    );

    store.erase_all_for(&other).await.unwrap();
}

async fn json_body(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Anonymous callers: GET is an empty note (store untouched), every write 401s — the
/// never-silently-anonymous policy the other per-account stores share.
#[tokio::test]
async fn anonymous_notes_read_empty_and_cannot_write() {
    let app = common::app_with(Path::new("__no_content__"), "http://127.0.0.1:9", None);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/notes?path=dsa/two-sum")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        json!({ "path": ["dsa", "two-sum"], "text": "", "updatedAt": null }),
        "anonymous reads an empty note"
    );

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/notes")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "path": ["dsa", "two-sum"], "text": "x" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "anonymous cannot save");

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "anonymous cannot erase");
}

/// A signed-in note past the limit is refused as a 400 naming its length — before the store is
/// reached, which is why this needs no database.
#[tokio::test]
async fn an_over_long_note_is_refused_before_the_store() {
    let issuer = common::stub_realm().await;
    let app = common::app_with_issuer(Path::new("__no_content__"), "http://127.0.0.1:9", None, &issuer);
    let token = common::mint(&issuer, "tester");

    let res = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/notes")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    json!({ "path": ["dsa", "two-sum"], "text": "x".repeat(NOTE_MAX_UTF16 + 1) }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    assert_eq!(body["error"], "Note is too long");
    assert!(
        body["detail"]
            .as_str()
            .unwrap()
            .contains(&(NOTE_MAX_UTF16 + 1).to_string())
    );
}
