//! Gated Postgres ITs (oracle: `PostgresSubmissionRepositoryIT`) — real database via
//! `docker compose up -d db` (:5532), migrations applied, rows cleaned after. Run:
//! `POSTGRES_IT=1 cargo test --test postgres_it`
//!
//! These run in CI on every push since step 45 (a `services: postgres` block), which is why
//! `--test-threads=1` is no longer in that command: each test now owns a namespace instead of
//! sharing one prefix, so the suite is safe under default parallelism.
//!
//! The crown piece: the FULL 202 → background judge → poll flow through the real router, the
//! real Postgres, and a local go-judge stub.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::routing::post;
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use synapse_server::insights::{LessonViewStore, PostgresLessonViews};
use synapse_server::submission::application::SubmissionRepository;
use synapse_server::submission::domain::{
    FailedCase, Submission, SubmissionId, SubmissionState, SuiteOutcome,
};
use synapse_server::submission::infrastructure::PostgresSubmissionRepository;
use synapse_shared::execution::RunStatus;
use tower::ServiceExt;
use uuid::Uuid;

const IT_PREFIX: &str = "it-rs";

/// Each test gets its OWN namespace under `it-rs`, and cleans only that.
///
/// This used to be a single shared `it-rs` prefix wiped by every `gated_pool()` call, which
/// meant two tests running concurrently deleted each other's fixtures —
/// `listing_is_newest_first_and_narrows_by_user` would see 2 rows where it inserted 3. It
/// never surfaced because the suite was env-gated and only ever run by hand, usually with
/// `--test-threads=1`. Wiring it into CI (step 45) is what exposed it: shared mutable state
/// under default parallelism is a flake waiting for a faster machine.
///
/// Callers pass a name unique to the test; `scoped_pool` derives the namespace from it.
async fn scoped_pool(scope: &str) -> Option<(PgPool, String)> {
    let pool = gated_pool().await?;
    let namespace = format!("{IT_PREFIX}-{scope}");
    // `lesson_path` is TEXT — the repository stores `path.join("/")`, not an array — so this
    // is a prefix match on the namespace rather than the old `like 'it-rs%'` that reached
    // into every other test's rows. Namespaces are distinct by construction
    // (`it-rs-roundtrip`, `it-rs-listing`, `it-rs-flow`), so no run can touch another's.
    sqlx::query("delete from submissions where lesson_path like $1")
        .bind(format!("{namespace}/%"))
        .execute(&pool)
        .await
        .unwrap();
    Some((pool, namespace))
}

/// The pool alone, for tests that own their rows by primary key rather than by lesson path —
/// the allowlist pair, which is keyed on a username it picks itself. They need no namespace
/// and must NOT wipe submissions.
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

fn submission_in(namespace: &str, path_tail: &str, state: SubmissionState) -> Submission {
    Submission {
        id: SubmissionId(Uuid::new_v4()),
        lesson_path: vec![namespace.to_owned(), path_tail.to_owned()],
        language: "python".to_owned(),
        source: "print(1)".to_owned(),
        user_id: None,
        created_at: Utc::now(),
        state,
    }
}

#[tokio::test]
async fn the_state_adt_flattens_and_reassembles_through_jsonb() {
    let Some((pool, ns)) = scoped_pool("roundtrip").await else {
        return;
    };
    let repo = PostgresSubmissionRepository::new(pool);

    let pending = submission_in(&ns, "roundtrip", SubmissionState::Pending);
    repo.save(&pending).await.unwrap();
    let stored = repo.get(pending.id).await.unwrap().unwrap();
    assert_eq!(stored.state, SubmissionState::Pending);
    assert_eq!(stored.lesson_path, pending.lesson_path, "the path splits back");
    assert_eq!(stored.user_id, None);

    repo.update(&pending.judging()).await.unwrap();
    let rejected = SuiteOutcome::Rejected {
        passed: 8,
        total: 118,
        first_failure: FailedCase {
            index: 8,
            args: BTreeMap::from([("n".to_owned(), "5".to_owned())]),
            expected: Some("120".to_owned()),
            stdout: "119\n".to_owned(),
            stderr: String::new(),
            status: RunStatus::Accepted,
        },
    };
    repo.update(&pending.completed(rejected.clone(), Utc::now()))
        .await
        .unwrap();
    let done = repo.get(pending.id).await.unwrap().unwrap();
    let SubmissionState::Completed { outcome, .. } = done.state else {
        panic!("must complete")
    };
    assert_eq!(outcome, rejected, "JSONB reassembles the exact ADT");
}

#[tokio::test]
async fn listing_is_newest_first_and_narrows_by_user() {
    let Some((pool, ns)) = scoped_pool("listing").await else {
        return;
    };
    let repo = PostgresSubmissionRepository::new(pool);

    let mut older = submission_in(&ns, "list", SubmissionState::Pending);
    older.created_at = Utc::now() - chrono::Duration::minutes(5);
    let newer = submission_in(&ns, "list", SubmissionState::Pending);
    let mut theirs = submission_in(&ns, "list", SubmissionState::Pending);
    theirs.user_id = Some("someone".to_owned());
    theirs.created_at = Utc::now() - chrono::Duration::minutes(1); // deterministic ordering
    for s in [&older, &newer, &theirs] {
        repo.save(s).await.unwrap();
    }

    let path = vec![ns.clone(), "list".to_owned()];
    let all = repo.list_for(&path, None).await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].id, newer.id, "newest first");
    let mine = repo.list_for(&path, Some("someone")).await.unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].id, theirs.id);

    assert!(
        repo.get(SubmissionId(Uuid::new_v4())).await.unwrap().is_none(),
        "unknown is None"
    );
}

/// The crown piece: POST 202 → the DETACHED judge runs against a go-judge stub → poll flips
/// pending/judging → completed accepted. Real router, real Postgres, real adapter chain.
#[tokio::test]
async fn the_full_submit_judge_poll_flow() {
    let Some((pool, ns)) = scoped_pool("flow").await else {
        return;
    };

    // A go-judge lookalike that always accepts with stdout "6\n" (both cases expect 6).
    let stub = Router::new().route(
        "/run",
        post(|| async {
            (
                [(header::CONTENT_TYPE, "application/json")],
                r#"[{"status":"Accepted","exitStatus":0,"files":{"stdout":"6\n","stderr":""}}]"#,
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let executor_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, stub).await;
    });

    // A real problem lesson: prose + a testcases fence (tier 2).
    let tmp = tempfile::tempdir().unwrap();
    let lesson_dir = tmp.path().join(format!("01-{ns}"));
    fs::create_dir_all(&lesson_dir).unwrap();
    fs::write(lesson_dir.join("book.json"), "{}").unwrap();
    fs::write(
        lesson_dir.join("01-flow.md"),
        "prose\n```testcases\n{\"args\":[{\"id\":\"n\",\"label\":\"N\",\"type\":\"int\"}],\
         \"cases\":[{\"args\":{\"n\":\"3\"},\"expected\":\"6\"},{\"args\":{\"n\":\"3\"},\"expected\":\"6\"}]}\n```",
    )
    .unwrap();

    let app = common::app_with(tmp.path(), &executor_url, Some(pool));

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "path": [&ns, "flow"], "language": "py", "source": "print(6)" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED, "the 202 contract");
    let bytes = axum::body::to_bytes(res.into_body(), 4096).await.unwrap();
    let id = serde_json::from_slice::<Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Poll until the detached judge lands the outcome.
    let mut last = Value::Null;
    for _ in 0..50 {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/submissions/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        last = serde_json::from_slice(&bytes).unwrap();
        if last["status"] == "completed" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(last["status"], "completed", "the judge must land: {last}");
    assert_eq!(last["verdict"], "accepted");
    assert_eq!(last["passed"], 2);
    assert_eq!(last["total"], 2);
}

#[tokio::test]
async fn the_allowlist_migration_seeds_the_dev_users() {
    use synapse_server::submission::application::SubmissionAllowlist;
    use synapse_server::submission::infrastructure::PostgresSubmissionAllowlist;
    let Some(pool) = gated_pool().await else { return };
    let allowlist = PostgresSubmissionAllowlist::new(pool);
    assert!(allowlist.is_allowed("tester").await.unwrap(), "seeded");
    assert!(allowlist.is_allowed("test1").await.unwrap(), "seeded");
    assert!(!allowlist.is_allowed("stranger").await.unwrap());
}

#[tokio::test]
async fn allowlist_grant_list_revoke_round_trip() {
    use synapse_server::submission::application::SubmissionAllowlist;
    use synapse_server::submission::infrastructure::PostgresSubmissionAllowlist;
    let Some(pool) = gated_pool().await else { return };
    let allowlist = PostgresSubmissionAllowlist::new(pool);
    // Clean slate for the IT-owned username.
    let _ = allowlist.revoke("it-rs-user").await;

    let granted = allowlist.grant("it-rs-user", Some("via IT")).await.unwrap();
    assert_eq!(granted.username, "it-rs-user");
    assert_eq!(granted.note.as_deref(), Some("via IT"));

    // Upsert refreshes the note in place.
    let regranted = allowlist.grant("it-rs-user", Some("refreshed")).await.unwrap();
    assert_eq!(regranted.note.as_deref(), Some("refreshed"));

    let listed = allowlist.list().await.unwrap();
    assert!(listed.iter().any(|e| e.username == "it-rs-user"));
    assert!(allowlist.is_allowed("it-rs-user").await.unwrap());

    assert!(allowlist.revoke("it-rs-user").await.unwrap());
    assert!(
        !allowlist.revoke("it-rs-user").await.unwrap(),
        "second revoke finds nothing"
    );
}

// ── Readership (step 49) ──────────────────────────────────────────────────────
// `lesson_view` is path-keyed like `submissions`, so it takes the same namespace treatment:
// clean only this test's own prefix, never a blanket `like 'it-rs%'`.

/// The pool with this test's `lesson_view` rows cleared — the readership twin of `scoped_pool`.
async fn views_pool(scope: &str) -> Option<(PgPool, String)> {
    let pool = gated_pool().await?;
    let namespace = format!("{IT_PREFIX}-{scope}");
    sqlx::query("delete from lesson_view where lesson_path like $1")
        .bind(format!("{namespace}/%"))
        .execute(&pool)
        .await
        .unwrap();
    Some((pool, namespace))
}

#[tokio::test]
async fn readership_counts_and_orders_by_views() {
    let Some((pool, ns)) = views_pool("views").await else {
        return;
    };
    let store = PostgresLessonViews::new(pool);

    let popular = format!("{ns}/popular");
    let quiet = format!("{ns}/quiet");
    for _ in 0..3 {
        store.record(&popular, false).await.unwrap();
    }
    store.record(&popular, true).await.unwrap();
    store.record(&quiet, false).await.unwrap();

    let top = store.top(50).await.unwrap();
    // Match on the namespace BOUNDARY, not the bare prefix. `it-rs-views-limit` starts with
    // `it-rs-views`, so a plain `starts_with(&ns)` pulls the sibling test's rows in whenever the
    // two run concurrently — which is the step-45 lesson repeating, one level down: the cleanup
    // was already boundary-aware (`{namespace}/%`) and only the assertion filter was not.
    let owned = format!("{ns}/");
    let mine: Vec<_> = top.iter().filter(|c| c.lesson_path.starts_with(&owned)).collect();

    assert_eq!(mine.len(), 2, "one row per distinct path, not per view");
    assert_eq!(mine[0].lesson_path, popular, "most-read first");
    assert_eq!(mine[0].views, 4);
    assert_eq!(
        mine[0].authed_views, 1,
        "only the bearer-carrying request counts as authed"
    );
    assert_eq!(mine[1].lesson_path, quiet);
    assert_eq!(mine[1].views, 1);
    assert_eq!(mine[1].authed_views, 0);
}

#[tokio::test]
async fn readership_limit_is_honoured() {
    let Some((pool, ns)) = views_pool("views-limit").await else {
        return;
    };
    let store = PostgresLessonViews::new(pool);
    for i in 0..3 {
        store.record(&format!("{ns}/lesson-{i}"), false).await.unwrap();
    }
    // The limit is applied by SQL over the whole table, so assert on the cap rather than on
    // this namespace's share of it — other suites' rows are legitimately present.
    assert!(store.top(2).await.unwrap().len() <= 2);
}
