//! Integration: a PRIVATE content source through the REAL stack — router, the catalog over two
//! mounted sources, the filesystem adapter, the identity verifier against a local JWKS stub.
//!
//! The shape under test is the one the binary boots with when `SYNAPSE_LOCAL_SOURCES` names a
//! satellite with `private: true` — a book served to its reader list only. What these pin is
//! every read path consulting the list: the lesson (401 / 403 / 200), the index, search and the
//! sitemap — and the rule that a PUBLIC read never verifies a token at all.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{mint, stub_realm, username};
use serde_json::Value;
use synapse_server::catalog::application::{Audience, Audiences, CatalogService, Placements};
use synapse_server::catalog::domain::content_tree::PRIMARY_SOURCE_ID;
use synapse_server::catalog::domain::merge::Placement;
use synapse_server::catalog::infrastructure::{FileSystemContentRepository, MountedSources, SourceRoot};
use tower::ServiceExt;

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// The spine holds a public book; the satellite is a root-is-the-book repository served to
/// `tester` alone. Both are mounted the way `main` mounts a local source, and the audience is
/// pinned the way `SYNAPSE_LOCAL_SOURCES` pins it.
fn app(issuer: &str) -> Router {
    let primary = tempfile::tempdir().unwrap();
    write(
        &primary.path().join("01-learn/category.json"),
        r#"{"title": "Learn"}"#,
    );
    write(
        &primary.path().join("01-learn/02-dsa/book.json"),
        r#"{"title": "DSA"}"#,
    );
    write(
        &primary.path().join("01-learn/02-dsa/01-intro.md"),
        "# Intro\nwelcome everyone",
    );

    let satellite = tempfile::tempdir().unwrap();
    write(
        &satellite.path().join("book.json"),
        r#"{"title": "Insight Earned", "slug": "dsa-guide-insight"}"#,
    );
    write(
        &satellite.path().join("01-sorting/01-selection-sort.md"),
        "---\ntitle: Selection sort, rewritten\n---\nthe reader's own explanation of xylophone ordering",
    );

    let mounted = MountedSources::new(vec![
        SourceRoot::new(PRIMARY_SOURCE_ID, primary.path()),
        SourceRoot::new("insight-earned", satellite.path()),
    ]);
    // Leaked on purpose: the router outlives this call and walks both directories for as long as
    // the test drives it. A test process is the one place that is simply free.
    std::mem::forget(primary);
    std::mem::forget(satellite);

    let placements = Placements::default();
    placements.publish(vec![Placement {
        source_id: "insight-earned".to_owned(),
        grouping: vec!["learn".to_owned()],
        order: Some(9),
    }]);
    let audiences = Audiences::pinned(BTreeMap::from([(
        "insight-earned".to_owned(),
        Audience::Private {
            readers: BTreeSet::from([username("tester")]),
        },
    )]));
    let catalog = CatalogService::with_placements(
        FileSystemContentRepository::mounted(mounted.clone(), true),
        placements,
    )
    .with_audiences(audiences);

    let mut deps = common::deps_with(Path::new("__no_content__"), "http://127.0.0.1:9", None, issuer);
    deps.catalog = Arc::new(catalog);
    deps.mounted = mounted;
    synapse_server::app(deps)
}

async fn get(app: Router, uri: &str, bearer: Option<&str>) -> (StatusCode, Value, String) {
    let mut builder = Request::builder().uri(uri);
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let res = app.oneshot(builder.body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        text,
    )
}

const PRIVATE_LESSON: &str = "/api/synapse/learn/dsa-guide-insight/sorting/selection-sort";
const PUBLIC_LESSON: &str = "/api/synapse/learn/dsa/intro";

#[tokio::test]
async fn a_private_lesson_answers_401_anonymous_403_unlisted_and_200_to_its_reader() {
    let issuer = stub_realm().await;

    let (status, body, _) = get(app(&issuer), PRIVATE_LESSON, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Sign in to read this book");
    assert!(
        body["detail"].as_str().unwrap().contains("dsa-guide-insight"),
        "the book is named: {body}"
    );

    let (status, body, _) = get(app(&issuer), PRIVATE_LESSON, Some(&mint(&issuer, "someone-else"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "This book is private");

    let (status, body, _) = get(app(&issuer), PRIVATE_LESSON, Some(&mint(&issuer, "tester"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["frontmatter"]["title"], "Selection sort, rewritten");
    assert!(body["raw"].as_str().unwrap().contains("xylophone"));
}

/// The rule that keeps the read path cheap: a public lesson never asks the verifier, so even a
/// bearer that could not verify is simply not looked at. A private lesson does ask, and a bearer
/// that fails there is 401 rather than silently anonymous.
#[tokio::test]
async fn a_public_lesson_never_verifies_a_token_and_a_private_one_always_does() {
    let issuer = stub_realm().await;
    let (status, body, _) = get(app(&issuer), PUBLIC_LESSON, Some("not-a-token")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["raw"].as_str().unwrap().contains("welcome"));

    let (status, body, _) = get(app(&issuer), PRIVATE_LESSON, Some("not-a-token")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Invalid bearer token", "{body}");
}

fn book_slugs(index: &Value) -> Vec<String> {
    fn walk(entries: &Value, out: &mut Vec<String>) {
        for entry in entries.as_array().unwrap_or(&Vec::new()) {
            match entry["kind"].as_str() {
                Some("book") => out.push(entry["slug"].as_str().unwrap().to_owned()),
                Some("category") => walk(&entry["entries"], out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&index["entries"], &mut out);
    out
}

fn book(index: &Value, slug: &str) -> Option<Value> {
    fn walk(entries: &Value, slug: &str) -> Option<Value> {
        for entry in entries.as_array()? {
            match entry["kind"].as_str() {
                Some("book") if entry["slug"] == slug => return Some(entry.clone()),
                Some("category") => {
                    if let Some(found) = walk(&entry["entries"], slug) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&index["entries"], slug)
}

#[tokio::test]
async fn the_index_carries_a_private_book_only_to_its_readers_and_marks_it_for_them() {
    let issuer = stub_realm().await;

    let (status, anonymous, _) = get(app(&issuer), "/api/synapse/index", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(book_slugs(&anonymous), vec!["dsa"]);
    assert!(
        !anonymous.to_string().contains("\"private\""),
        "the public tree is untouched: {anonymous}"
    );

    let (_, unlisted, _) = get(
        app(&issuer),
        "/api/synapse/index",
        Some(&mint(&issuer, "someone-else")),
    )
    .await;
    assert_eq!(book_slugs(&unlisted), vec!["dsa"]);

    let (_, reader, _) = get(app(&issuer), "/api/synapse/index", Some(&mint(&issuer, "tester"))).await;
    assert_eq!(book_slugs(&reader), vec!["dsa", "dsa-guide-insight"]);
    assert_eq!(
        book(&reader, "dsa-guide-insight").unwrap()["private"],
        true,
        "a lock for the rail"
    );
    assert!(
        book(&reader, "dsa").unwrap().get("private").is_none(),
        "public books carry no mark"
    );

    // A bearer on the index IS verified — present means "may be a reader", and a bad one is a
    // bad one, never an anonymous fallback.
    let (status, _, _) = get(app(&issuer), "/api/synapse/index", Some("not-a-token")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn search_and_the_sitemap_keep_a_private_book_to_its_readers() {
    let issuer = stub_realm().await;
    let query = "/api/synapse/search?q=xylophone";

    let (status, anonymous, _) = get(app(&issuer), query, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(anonymous["results"].as_array().unwrap().is_empty(), "{anonymous}");

    let (_, unlisted, _) = get(app(&issuer), query, Some(&mint(&issuer, "someone-else"))).await;
    assert!(unlisted["results"].as_array().unwrap().is_empty(), "{unlisted}");

    let (_, reader, _) = get(app(&issuer), query, Some(&mint(&issuer, "tester"))).await;
    assert_eq!(reader["results"].as_array().unwrap().len(), 1, "{reader}");
    assert_eq!(reader["results"][0]["bookSlug"], "dsa-guide-insight");

    // Public text still searches for everyone, bearer or not.
    let (_, public, _) = get(app(&issuer), "/api/synapse/search?q=welcome", None).await;
    assert_eq!(public["results"].as_array().unwrap().len(), 1);

    let (status, _, sitemap) = get(app(&issuer), "/sitemap.xml", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(sitemap.contains("learn/dsa/intro"), "{sitemap}");
    assert!(
        !sitemap.contains("dsa-guide-insight"),
        "a crawler is nobody's reader: {sitemap}"
    );
}
