//! Covers real temp dirs, drafts, traversal, the watermark.

#![allow(clippy::unwrap_used)]

use super::*;

fn blog_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let blog = dir.path().join("blog");
    std::fs::create_dir(&blog).unwrap();
    std::fs::write(blog.join("hello.md"), "# Hello").unwrap();
    std::fs::write(blog.join("world.md"), "# World").unwrap();
    std::fs::write(blog.join("_draft.md"), "# Draft").unwrap();
    std::fs::write(dir.path().join("secret.txt"), "no").unwrap();
    dir
}

#[tokio::test]
async fn load_all_returns_each_md_and_skips_drafts() {
    let dir = blog_dir();
    let repo = FileSystemBlogRepository::new(dir.path(), true);
    let posts = repo.load_all().await.unwrap();
    let slugs: Vec<&str> = posts.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(slugs, vec!["hello", "world"], "_draft.md is skipped");
    assert_eq!(posts[0].1, "# Hello");
}

#[tokio::test]
async fn read_fetches_one_post_and_unknown_is_not_found() {
    let dir = blog_dir();
    let repo = FileSystemBlogRepository::new(dir.path(), true);
    assert_eq!(repo.read("hello").await.unwrap(), "# Hello");
    assert_eq!(
        repo.read("ghost").await.unwrap_err(),
        BlogError::NotFound("ghost".to_owned())
    );
}

#[tokio::test]
async fn a_traversal_shaped_slug_never_escapes_the_blog_dir() {
    let dir = blog_dir();
    let repo = FileSystemBlogRepository::new(dir.path(), true);
    assert_eq!(
        repo.read("../secret").await.unwrap_err(),
        BlogError::NotFound("../secret".to_owned())
    );
}

#[tokio::test]
async fn the_watermark_advances_when_a_post_is_added() {
    let dir = blog_dir();
    let repo = FileSystemBlogRepository::new(dir.path(), true);
    let before = repo.version().await;
    // A new file changes the count even when mtime granularity is coarse.
    std::fs::write(dir.path().join("blog/brand-new.md"), "# New").unwrap();
    assert_ne!(repo.version().await, before);

    let frozen = FileSystemBlogRepository::new(dir.path(), false);
    assert_eq!(frozen.version().await, "static", "prod pins the listing");
}
