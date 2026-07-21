//! Covers the four frontmatter parse behaviors.

#![allow(clippy::unwrap_used)]

use super::*;

const FULL: &str = "---\n\
title: Rebuilding Cortex\n\
summary: Why a rewrite\n\
publishedAt: 2026-06-01\n\
tags: [meta, scala]\n\
readMinutes: 8\n\
eyebrow: Meta\n\
---\n\
# Rebuilding Cortex\n\nBody text.";

#[test]
fn a_full_fence_maps_every_field_and_strips_the_fence() {
    let post = BlogPost::parse("rebuilding-cortex", FULL);
    assert_eq!(post.title, "Rebuilding Cortex");
    assert_eq!(post.summary.as_deref(), Some("Why a rewrite"));
    assert_eq!(
        post.published_at,
        Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap())
    );
    assert_eq!(post.tags, vec!["meta", "scala"]);
    assert_eq!(post.read_minutes, Some(8));
    assert_eq!(post.eyebrow.as_deref(), Some("Meta"));
    assert!(post.body.starts_with("# Rebuilding Cortex"));
    assert!(!post.body.contains("publishedAt"));
}

#[test]
fn no_fence_means_slug_title_and_whole_content_body() {
    let post = BlogPost::parse("plain", "Just some text.\nNo fence here.");
    assert_eq!(post.title, "plain");
    assert_eq!(post.body, "Just some text.\nNo fence here.");
    assert_eq!(post.published_at, None);
}

#[test]
fn malformed_date_and_read_minutes_degrade_to_none() {
    let raw = "---\ntitle: Ok\npublishedAt: someday\nreadMinutes: a few\n---\nbody";
    let post = BlogPost::parse("p", raw);
    assert_eq!(post.title, "Ok");
    assert_eq!(post.published_at, None);
    assert_eq!(post.read_minutes, None);
}

#[test]
fn summary_view_drops_the_body_but_keeps_the_card_fields() {
    let post = BlogPost::parse("rebuilding-cortex", FULL);
    let card = post.summary_view();
    assert_eq!(card.slug, post.slug);
    assert_eq!(card.title, post.title);
    assert_eq!(card.published_at, post.published_at);
    assert_eq!(card.tags, post.tags);
}
