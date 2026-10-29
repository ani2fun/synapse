//! Integration: `/content-assets` — the `_assets/_simulators/` folder beside a lesson (or at a
//! book's root), over the real router and a real two-source catalog: slug path → folder, the
//! directory redirect, only `_simulators` and public images published (ADR-RS013), the traversal
//! and symlink guards, `no-store` on a miss, and a PRIVATE source's files gated exactly like its
//! prose and its `/media` (401 / 403 / 200).

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
use synapse_server::catalog::application::{Audience, Audiences, CatalogService, Placements};
use synapse_server::catalog::domain::content_tree::PRIMARY_SOURCE_ID;
use synapse_server::catalog::domain::merge::Placement;
use synapse_server::catalog::infrastructure::{FileSystemContentRepository, MountedSources, SourceRoot};
use tower::ServiceExt;

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A public book in the spine (a lesson inside a chapter, a book-level runtime) and a private
/// root-is-the-book satellite served to `tester` alone — mounted the way `main` mounts them.
fn app(issuer: &str) -> Router {
    let primary = tempfile::tempdir().unwrap();
    let p = primary.path();
    write(&p.join("01-learn/category.json"), r#"{"title": "Learn"}"#);
    write(&p.join("01-learn/02-dsa/book.json"), r#"{"title": "DSA"}"#);
    write(
        &p.join("01-learn/02-dsa/01-basics/01-intro.md"),
        "# Intro\nwelcome",
    );
    write(
        &p.join("01-learn/02-dsa/01-basics/_assets/_simulators/index.html"),
        "<p>figures</p>",
    );
    write(
        &p.join("01-learn/02-dsa/01-basics/_assets/_simulators/figures.js"),
        "draw()",
    );
    write(
        &p.join("01-learn/02-dsa/01-basics/_assets/_diagrams/a.d2"),
        "a -> b",
    );
    write(
        &p.join("01-learn/02-dsa/_assets/_simulators/runtime/figures.js"),
        "runtime()",
    );
    write(&p.join("01-learn/02-dsa/01-basics/_assets/flow.svg"), "<svg/>");
    write(&p.join("01-learn/02-dsa/01-basics/_assets/flow.d2"), "a -> b");
    write(
        &p.join("01-learn/02-dsa/01-basics/_assets/screens/login.png"),
        "png bytes",
    );
    write(
        &p.join("01-learn/02-dsa/01-basics/_assets/_drafts/old.png"),
        "draft",
    );
    write(
        &p.join("01-learn/02-dsa/01-basics/_assets/screens.png/inner.txt"),
        "a folder",
    );
    // A link that names an image but points at the lesson's own prose.
    std::os::unix::fs::symlink(
        p.join("01-learn/02-dsa/01-basics/01-intro.md"),
        p.join("01-learn/02-dsa/01-basics/_assets/escape.png"),
    )
    .unwrap();

    let satellite = tempfile::tempdir().unwrap();
    let s = satellite.path();
    write(
        &s.join("book.json"),
        r#"{"title": "Insight Earned", "slug": "dsa-guide-insight"}"#,
    );
    write(
        &s.join("01-sorting/01-selection-sort.md"),
        "---\ntitle: Selection sort\n---\nprivate prose",
    );
    write(
        &s.join("01-sorting/_assets/_simulators/index.html"),
        "<p>private figure</p>",
    );
    write(&s.join("01-sorting/_assets/sorted.png"), "private png");

    let mounted = MountedSources::new(vec![
        SourceRoot::new(PRIMARY_SOURCE_ID, p),
        SourceRoot::new("insight-earned", s),
    ]);
    // Leaked on purpose: the router walks both directories for as long as the test drives it.
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
    .with_audiences(audiences.clone());

    let mut deps = common::deps_with(Path::new("__no_content__"), "http://127.0.0.1:9", None, issuer);
    deps.catalog = Arc::new(catalog);
    deps.mounted = mounted;
    deps.audiences = audiences;
    synapse_server::app(deps)
}

struct Reply {
    status: StatusCode,
    content_type: String,
    cache: String,
    location: String,
    body: String,
}

async fn get(app: Router, uri: &str, bearer: Option<&str>) -> Reply {
    let mut builder = Request::builder().uri(uri);
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let res = app.oneshot(builder.body(Body::empty()).unwrap()).await.unwrap();
    let header_of = |name| {
        res.headers()
            .get(name)
            .map(|v| v.to_str().unwrap().to_owned())
            .unwrap_or_default()
    };
    let (status, content_type, cache, location) = (
        res.status(),
        header_of(header::CONTENT_TYPE),
        header_of(header::CACHE_CONTROL),
        header_of(header::LOCATION),
    );
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    Reply {
        status,
        content_type,
        cache,
        location,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

const LESSON: &str = "/content-assets/learn/dsa/basics/intro/_assets";

#[tokio::test]
async fn a_lessons_simulator_is_served_from_the_folder_beside_it() {
    let issuer = stub_realm().await;
    let r = get(app(&issuer), &format!("{LESSON}/_simulators/figures.js"), None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.content_type, "text/javascript");
    assert_eq!(r.cache, "public, max-age=60");
    assert_eq!(r.body, "draw()");
}

#[tokio::test]
async fn a_book_path_serves_the_books_own_assets() {
    let issuer = stub_realm().await;
    let r = get(
        app(&issuer),
        "/content-assets/learn/dsa/_assets/_simulators/runtime/figures.js",
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body, "runtime()");
}

#[tokio::test]
async fn the_folder_redirects_to_its_slash_form_then_serves_its_index() {
    let issuer = stub_realm().await;
    let r = get(app(&issuer), &format!("{LESSON}/_simulators"), None).await;
    assert_eq!(r.status, StatusCode::MOVED_PERMANENTLY);
    assert_eq!(r.location, format!("{LESSON}/_simulators/"));
    let r = get(app(&issuer), &format!("{LESSON}/_simulators/"), None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.content_type, "text/html; charset=utf-8");
    assert_eq!(r.body, "<p>figures</p>");
}

#[tokio::test]
async fn only_simulators_are_published_and_a_miss_is_never_cached() {
    let issuer = stub_realm().await;
    for uri in [
        format!("{LESSON}/_diagrams/a.d2"),         // exists, but not published
        format!("{LESSON}/_simulators/nothing.js"), // absent
        "/content-assets/learn/dsa/nope/_assets/_simulators/index.html".to_owned(), // no such lesson
        format!("{LESSON}/_simulators/../../01-intro.md"), // the lesson's own prose
        format!("{LESSON}/_simulators/%2e%2e/_diagrams/a.d2"), // encoded traversal
    ] {
        let r = get(app(&issuer), &uri, None).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(r.cache, "no-store", "{uri}");
    }
}

#[tokio::test]
async fn a_private_books_assets_go_to_its_readers_only() {
    let issuer = stub_realm().await;
    let uri = "/content-assets/learn/dsa-guide-insight/sorting/selection-sort/_assets/_simulators/index.html";

    let anonymous = get(app(&issuer), uri, None).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    assert_eq!(anonymous.cache, "private, no-store");
    assert!(!anonymous.body.contains("private figure"));

    let stranger = get(app(&issuer), uri, Some(&mint(&issuer, "stranger"))).await;
    assert_eq!(stranger.status, StatusCode::FORBIDDEN);
    assert!(!stranger.body.contains("private figure"));

    let reader = get(app(&issuer), uri, Some(&mint(&issuer, "tester"))).await;
    assert_eq!(reader.status, StatusCode::OK);
    assert_eq!(reader.cache, "private, no-store");
    assert_eq!(reader.body, "<p>private figure</p>");
}

#[tokio::test]
async fn an_image_beside_a_lesson_is_served_with_its_type() {
    let issuer = stub_realm().await;
    let r = get(app(&issuer), &format!("{LESSON}/flow.svg"), None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.content_type, "image/svg+xml");
    assert_eq!(r.cache, "public, max-age=60");
    assert_eq!(r.body, "<svg/>");

    let r = get(app(&issuer), &format!("{LESSON}/screens/login.png"), None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.content_type, "image/png");
    assert_eq!(r.body, "png bytes");
}

#[tokio::test]
async fn only_images_outside_hidden_folders_are_published_beside_a_lesson() {
    let issuer = stub_realm().await;
    for uri in [
        format!("{LESSON}/flow.d2"),                 // the diagram source beside its image
        format!("{LESSON}/_drafts/old.png"),         // an image in an underscore folder
        format!("{LESSON}/escape.png"),              // a symlink out of `_assets/`
        format!("{LESSON}/screens.png"),             // a folder named like an image
        format!("{LESSON}/missing.png"),             // absent
        format!("{LESSON}/screens/%2e%2e/flow.svg"), // encoded traversal
    ] {
        let r = get(app(&issuer), &uri, None).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(r.cache, "no-store", "{uri}");
    }
}

#[tokio::test]
async fn a_private_books_images_go_to_its_readers_only() {
    let issuer = stub_realm().await;
    let uri = "/content-assets/learn/dsa-guide-insight/sorting/selection-sort/_assets/sorted.png";

    let anonymous = get(app(&issuer), uri, None).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    assert!(!anonymous.body.contains("private png"));

    let stranger = get(app(&issuer), uri, Some(&mint(&issuer, "stranger"))).await;
    assert_eq!(stranger.status, StatusCode::FORBIDDEN);

    let reader = get(app(&issuer), uri, Some(&mint(&issuer, "tester"))).await;
    assert_eq!(reader.status, StatusCode::OK);
    assert_eq!(reader.cache, "private, no-store");
    assert_eq!(reader.body, "private png");
}
