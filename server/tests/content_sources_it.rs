//! Gated Postgres IT for the source registry — the SQL the admin panel drives.
//! Run: `POSTGRES_IT=1 cargo test --test content_sources_it`
//!
//! Each test owns its own repo namespace and cleans only that, so the suite is safe under
//! default parallelism.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sqlx::PgPool;
use synapse_server::catalog::application::{ContentSourceDraft, ContentSources, SyncOutcome};
use synapse_server::catalog::infrastructure::PostgresContentSources;

async fn gated_pool(namespace: &str) -> Option<PgPool> {
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
    sqlx::query("delete from content_source where repo like $1")
        .bind(format!("{namespace}/%"))
        .execute(&pool)
        .await
        .unwrap();
    Some(pool)
}

fn draft(repo: &str, grouping: &str, order: Option<i32>) -> ContentSourceDraft {
    ContentSourceDraft::register(repo, None, Some(grouping), order, None).unwrap()
}

#[tokio::test]
async fn a_registration_round_trips_with_its_placement() {
    let namespace = "it-src-a";
    let Some(pool) = gated_pool(namespace).await else {
        return;
    };
    let registry = PostgresContentSources::new(pool);

    let stored = registry
        .upsert(&draft(
            &format!("{namespace}/java-guide"),
            "programming-languages",
            Some(7),
        ))
        .await
        .unwrap();

    assert_eq!(stored.id, "java-guide");
    assert_eq!(stored.grouping, vec!["programming-languages".to_owned()]);
    assert_eq!(stored.order, Some(7));
    assert!(stored.enabled);
    assert_eq!(stored.last_sha, None, "nothing has been fetched yet");

    let placement = stored.placement();
    assert_eq!(placement.grouping, vec!["programming-languages".to_owned()]);

    assert!(registry.remove("java-guide").await.unwrap());
    assert!(
        !registry.remove("java-guide").await.unwrap(),
        "removing twice is not an error, just false"
    );
}

/// Re-registering EDITS the row rather than adding a second — and must not discard the checkout
/// that is already serving.
#[tokio::test]
async fn re_registering_edits_in_place_and_keeps_the_sync_state() {
    let namespace = "it-src-b";
    let Some(pool) = gated_pool(namespace).await else {
        return;
    };
    let registry = PostgresContentSources::new(pool);
    let repo = format!("{namespace}/python-guide");

    registry.upsert(&draft(&repo, "", None)).await.unwrap();
    registry
        .record_sync("python-guide", &SyncOutcome::Landed("abc123".to_owned()))
        .await
        .unwrap();

    let moved = registry
        .upsert(&draft(&repo, "programming-languages", Some(6)))
        .await
        .unwrap();

    assert_eq!(moved.grouping, vec!["programming-languages".to_owned()]);
    assert_eq!(moved.order, Some(6));
    assert_eq!(
        moved.last_sha.as_deref(),
        Some("abc123"),
        "a placement change must not blank the book until the next tick"
    );

    let mine: Vec<_> = registry
        .list()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.repo == repo)
        .collect();
    assert_eq!(mine.len(), 1, "one row per repository");

    registry.remove("python-guide").await.unwrap();
}

/// A failed fetch keeps the last good sha: stale content beats an empty book.
#[tokio::test]
async fn a_failure_records_the_error_and_keeps_the_last_good_commit() {
    let namespace = "it-src-c";
    let Some(pool) = gated_pool(namespace).await else {
        return;
    };
    let registry = PostgresContentSources::new(pool);
    let repo = format!("{namespace}/dsa-guide");

    registry.upsert(&draft(&repo, "", None)).await.unwrap();
    registry
        .record_sync("dsa-guide", &SyncOutcome::Landed("good-sha".to_owned()))
        .await
        .unwrap();
    registry
        .record_sync("dsa-guide", &SyncOutcome::Failed("404 from GitHub".to_owned()))
        .await
        .unwrap();

    let row = registry
        .list()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.repo == repo)
        .unwrap();
    assert_eq!(row.last_sha.as_deref(), Some("good-sha"));
    assert_eq!(row.last_error.as_deref(), Some("404 from GitHub"));
    assert!(row.last_synced_at.is_some());

    // And a later success clears the error.
    registry
        .record_sync("dsa-guide", &SyncOutcome::Landed("newer-sha".to_owned()))
        .await
        .unwrap();
    let row = registry
        .list()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.repo == repo)
        .unwrap();
    assert_eq!(row.last_sha.as_deref(), Some("newer-sha"));
    assert_eq!(row.last_error, None);

    registry.remove("dsa-guide").await.unwrap();
}

/// Mount order decides the merge's first-wins rule, so it must be stable and enabled-first.
#[tokio::test]
async fn listing_is_enabled_first_then_configured_order() {
    let namespace = "it-src-d";
    let Some(pool) = gated_pool(namespace).await else {
        return;
    };
    let registry = PostgresContentSources::new(pool);

    registry
        .upsert(&draft(&format!("{namespace}/zeta-guide"), "", Some(9)))
        .await
        .unwrap();
    registry
        .upsert(&draft(&format!("{namespace}/alpha-guide"), "", Some(1)))
        .await
        .unwrap();
    let off = ContentSourceDraft::register(
        &format!("{namespace}/off-guide"),
        None,
        None,
        Some(0),
        Some(false),
    )
    .unwrap();
    registry.upsert(&off).await.unwrap();

    let ids: Vec<String> = registry
        .list()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.repo.starts_with(namespace))
        .map(|r| r.id)
        .collect();
    assert_eq!(
        ids,
        vec![
            "alpha-guide".to_owned(),
            "zeta-guide".to_owned(),
            "off-guide".to_owned()
        ],
        "a disabled source sorts last however low its order"
    );

    for id in ["zeta-guide", "alpha-guide", "off-guide"] {
        registry.remove(id).await.unwrap();
    }
}
