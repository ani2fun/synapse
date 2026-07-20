//! The blog domain (oracle: `BlogPost` + `BlogFrontmatter`): one post per markdown file, a
//! lenient frontmatter fence, and graceful degradation — a malformed date or read-minutes value
//! becomes `None`, never an error. The fence MECHANISM was a deliberate byte-identical twin of
//! the catalog's until step 62; it is `platform::frontmatter` now, and what this module owns
//! is blog's VOCABULARY — which fields a post has and how they degrade.

use chrono::NaiveDate;

use crate::platform::frontmatter::{fields_and_body, parse_inline_list};

/// One published post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPost {
    pub slug: String,
    pub title: String,
    pub summary: Option<String>,
    pub published_at: Option<NaiveDate>,
    pub tags: Vec<String>,
    pub read_minutes: Option<i32>,
    pub eyebrow: Option<String>,
    pub body: String,
}

/// The listing card — every field of the post except the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogSummary {
    pub slug: String,
    pub title: String,
    pub summary: Option<String>,
    pub published_at: Option<NaiveDate>,
    pub tags: Vec<String>,
    pub read_minutes: Option<i32>,
    pub eyebrow: Option<String>,
}

impl BlogPost {
    /// Parse one raw markdown file. The slug is the fallback title; unparseable metadata
    /// degrades field-by-field.
    #[must_use]
    pub fn parse(slug: &str, raw: &str) -> Self {
        let (fields, body) = fields_and_body(raw);
        Self {
            slug: slug.to_owned(),
            title: fields.get("title").cloned().unwrap_or_else(|| slug.to_owned()),
            summary: fields.get("summary").cloned(),
            published_at: fields
                .get("publishedAt")
                .and_then(|d| NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d").ok()),
            tags: fields
                .get("tags")
                .map(|v| parse_inline_list(v))
                .unwrap_or_default(),
            read_minutes: fields.get("readMinutes").and_then(|m| m.trim().parse().ok()),
            eyebrow: fields.get("eyebrow").cloned(),
            body,
        }
    }

    #[must_use]
    pub fn summary_view(&self) -> BlogSummary {
        BlogSummary {
            slug: self.slug.clone(),
            title: self.title.clone(),
            summary: self.summary.clone(),
            published_at: self.published_at,
            tags: self.tags.clone(),
            read_minutes: self.read_minutes,
            eyebrow: self.eyebrow.clone(),
        }
    }
}

// The fence splitter and inline-list parser are `platform::frontmatter` since step 62 —
// this module keeps blog's VOCABULARY (title/summary/publishedAt/tags/readMinutes/eyebrow).

#[cfg(test)]
#[path = "blog_tests.rs"]
mod tests;
