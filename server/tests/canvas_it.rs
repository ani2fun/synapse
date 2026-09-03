//! Saved design-canvas ITs. The store half is gated Postgres (`POSTGRES_IT=1`, db on :5532, the
//! `postgres_it.rs` convention): the `PostgresCanvasStore` adapter's SQL, the jsonb round trip,
//! and the ownership rule that keeps one account's plan out of another's reach. The HTTP half is
//! ungated — every anonymous path short-circuits before a store touch, so the lazy pool is never
//! dialed.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use synapse_server::canvas::{CanvasError, CanvasStore, PostgresCanvasStore};
use synapse_shared::canvas::{CanvasBodyDto, CanvasIdeaDto};
use tower::ServiceExt;

const IT_PREFIX: &str = "it-rs-canvas";

/// A gated pool with THIS test's `canvas_entries` rows cleared. Each test owns a distinct
/// `user_id` namespace, so the suite is safe under default parallelism (the `postgres_it` lesson).
async fn canvas_pool(scope: &str) -> Option<(PgPool, String)> {
    let pool = gated_pool().await?;
    let user = format!("{IT_PREFIX}-{scope}");
    sqlx::query("delete from canvas_entries where user_id like $1")
        .bind(format!("{user}%"))
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

fn a_body(problem: &str) -> CanvasBodyDto {
    CanvasBodyDto {
        problem: problem.to_owned(),
        constraints: "· max N — 1e4".to_owned(),
        ret: "int[] of two indices".to_owned(),
        ideas: vec![CanvasIdeaDto {
            name: "Hash map".to_owned(),
            description: "one pass, complements in a map".to_owned(),
            time: "O(n)".to_owned(),
            space: "O(n)".to_owned(),
        }],
        ..CanvasBodyDto::default()
    }
}

/// The jsonb round trip, in full: a body written and read back is the SAME body, ideas included.
/// A canvas that loses its ideas on the way through the store is worse than one that fails loudly.
#[tokio::test]
async fn an_entry_round_trips_through_jsonb_and_lists_newest_first() {
    let Some((pool, user)) = canvas_pool("roundtrip").await else {
        return;
    };
    let store = PostgresCanvasStore::new(pool);

    let first = store.save(&user, "dsa/two-sum", &a_body("older")).await.unwrap();
    let second = store.save(&user, "dsa/two-sum", &a_body("newer")).await.unwrap();

    let listed = store.list_for(&user, "dsa/two-sum").await.unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, second.id, "newest first");
    assert_eq!(listed[1].id, first.id);

    let body = &listed[0].body;
    assert_eq!(body.problem, "newer");
    assert_eq!(
        body.ret, "int[] of two indices",
        "`ret` survives its `return` rename"
    );
    assert_eq!(body.ideas.len(), 1);
    assert_eq!(body.ideas[0].time, "O(n)");
    assert_eq!(
        listed[0].path,
        vec!["dsa".to_owned(), "two-sum".to_owned()],
        "the stored `/`-joined path splits back into segments"
    );
}

/// Entries are scoped to ONE problem: a canvas written for two-sum must not appear under a
/// different problem, or the Think pane would show the reader someone else's plan for this page.
#[tokio::test]
async fn entries_are_scoped_to_one_problem() {
    let Some((pool, user)) = canvas_pool("scoped").await else {
        return;
    };
    let store = PostgresCanvasStore::new(pool);

    store
        .save(&user, "dsa/two-sum", &a_body("for two-sum"))
        .await
        .unwrap();
    store
        .save(&user, "dsa/reverse", &a_body("for reverse"))
        .await
        .unwrap();

    let listed = store.list_for(&user, "dsa/two-sum").await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].body.problem, "for two-sum");
}

/// The ownership rule. "No such entry" and "not yours" stay DISTINCT so the edge can answer 404
/// and 403 honestly — and neither one deletes.
#[tokio::test]
async fn delete_is_owner_only_and_distinguishes_missing_from_forbidden() {
    let Some((pool, user)) = canvas_pool("owner").await else {
        return;
    };
    let store = PostgresCanvasStore::new(pool);
    let stranger = format!("{user}-stranger");

    let entry = store.save(&user, "dsa/two-sum", &a_body("mine")).await.unwrap();

    assert_eq!(
        store.delete(&stranger, &entry.id).await,
        Err(CanvasError::NotYours(entry.id.clone())),
        "someone else's entry is forbidden, not missing"
    );
    assert_eq!(
        store.list_for(&user, "dsa/two-sum").await.unwrap().len(),
        1,
        "and the refusal did not delete it"
    );

    let ghost = uuid::Uuid::new_v4().to_string();
    assert_eq!(
        store.delete(&user, &ghost).await,
        Err(CanvasError::NotFound(ghost))
    );
    // A malformed id is "no such entry", not a 500: the route hands the raw path segment through.
    assert!(matches!(
        store.delete(&user, "not-a-uuid").await,
        Err(CanvasError::NotFound(_))
    ));

    store.delete(&user, &entry.id).await.unwrap();
    assert!(store.list_for(&user, "dsa/two-sum").await.unwrap().is_empty());
}

/// Erase-all is the account page's leg: it clears every problem's entries for THIS user and
/// nobody else's.
#[tokio::test]
async fn erase_all_clears_this_user_across_problems_only() {
    let Some((pool, user)) = canvas_pool("erase").await else {
        return;
    };
    let store = PostgresCanvasStore::new(pool);
    let other = format!("{user}-other");

    store.save(&user, "dsa/two-sum", &a_body("a")).await.unwrap();
    store.save(&user, "dsa/reverse", &a_body("b")).await.unwrap();
    store
        .save(&other, "dsa/two-sum", &a_body("theirs"))
        .await
        .unwrap();

    assert_eq!(store.erase_all_for(&user).await.unwrap(), 2);
    assert!(store.list_for(&user, "dsa/two-sum").await.unwrap().is_empty());
    assert!(store.list_for(&user, "dsa/reverse").await.unwrap().is_empty());
    assert_eq!(
        store.list_for(&other, "dsa/two-sum").await.unwrap().len(),
        1,
        "another account's canvases are untouched"
    );

    store.erase_all_for(&other).await.unwrap();
}

/// Anonymous callers: GET is `[]` (store untouched), every write 401s — the
/// never-silently-anonymous policy `progress` and `submission` share. Ungated: nothing here
/// reaches the store.
#[tokio::test]
async fn anonymous_canvas_lists_empty_and_cannot_write() {
    let app = common::app_with(Path::new("__no_content__"), "http://127.0.0.1:9", None);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/canvas?path=dsa/two-sum")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 4096).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body, json!([]), "anonymous sees an empty list");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/canvas")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "path": ["dsa", "two-sum"], "body": { "problem": "x" } }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "anonymous cannot save");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/canvas")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "anonymous cannot erase");

    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/canvas/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "anonymous cannot delete — and never learns whether the id exists"
    );
}

/// A half-filled canvas is the NORMAL state of one being worked on, so a body carrying only some
/// areas must decode rather than 400. `serde(default)` is what makes that true; this pins it.
#[test]
fn a_partial_body_decodes_to_empty_areas() {
    let body: CanvasBodyDto = serde_json::from_value(json!({ "problem": "only this" })).unwrap();
    assert_eq!(body.problem, "only this");
    assert_eq!(body.constraints, "");
    assert!(body.ideas.is_empty());

    // And an empty object is a legal canvas — the reader has opened Think and typed nothing yet.
    let empty: CanvasBodyDto = serde_json::from_value(json!({})).unwrap();
    assert_eq!(empty, CanvasBodyDto::default());
}
