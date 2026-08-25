//! Covers ordering, the version-gated cache, neighbours, `NotFound`.

#![allow(clippy::unwrap_used)]

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

/// A fixed-version repo that counts `load_all` calls (the cache proof).
struct FakeRepo {
    posts: Vec<(String, String)>,
    loads: AtomicUsize,
}

impl FakeRepo {
    fn new() -> Self {
        let post = |slug: &str, date: Option<&str>| {
            let fence = date.map_or(String::new(), |d| format!("---\npublishedAt: {d}\n---\n"));
            (slug.to_owned(), format!("{fence}# {slug}\nbody"))
        };
        Self {
            posts: vec![
                post("oldest", Some("2026-01-01")),
                post("newest", Some("2026-06-01")),
                post("undated", None),
                post("mid", Some("2026-03-01")),
            ],
            loads: AtomicUsize::new(0),
        }
    }
}

impl BlogRepository for &FakeRepo {
    async fn version(&self) -> String {
        "v1".to_owned()
    }

    async fn load_all(&self) -> Result<Vec<(String, String)>, BlogError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(self.posts.clone())
    }

    async fn read(&self, slug: &str) -> Result<String, BlogError> {
        self.posts
            .iter()
            .find(|(s, _)| s == slug)
            .map(|(_, raw)| raw.clone())
            .ok_or_else(|| BlogError::NotFound(slug.to_owned()))
    }
}

#[tokio::test]
async fn list_is_newest_first_undated_last() {
    let repo = FakeRepo::new();
    let service = BlogService::new(&repo);
    let slugs: Vec<String> = service
        .list()
        .await
        .unwrap()
        .iter()
        .map(|s| s.slug.clone())
        .collect();
    assert_eq!(slugs, vec!["newest", "mid", "oldest", "undated"]);
}

#[tokio::test]
async fn the_version_gated_cache_builds_the_listing_once() {
    let repo = FakeRepo::new();
    let service = BlogService::new(&repo);
    service.list().await.unwrap();
    service.list().await.unwrap();
    service.post("mid").await.unwrap();
    assert_eq!(repo.loads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_post_carries_publish_order_neighbours() {
    let repo = FakeRepo::new();
    let service = BlogService::new(&repo);
    let mid = service.post("mid").await.unwrap();
    assert_eq!(mid.prev.as_deref(), Some("oldest"), "prev = older");
    assert_eq!(mid.next.as_deref(), Some("newest"), "next = newer");
    let newest = service.post("newest").await.unwrap();
    assert_eq!(newest.next, None, "the newest has nothing newer");
    let oldest = service.post("oldest").await.unwrap();
    assert_eq!(
        oldest.prev.as_deref(),
        Some("undated"),
        "undated sits below oldest"
    );
}

#[tokio::test]
async fn an_unknown_slug_is_not_found() {
    let repo = FakeRepo::new();
    let service = BlogService::new(&repo);
    assert_eq!(
        service.post("ghost").await.unwrap_err(),
        BlogError::NotFound("ghost".to_owned())
    );
}
