//! Tests for `FsLessonSource` — hermetic, over an in-memory content repo with the REAL numbered
//! directory shape, because the walker map is the only correct path from a URL slug to a file.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use crate::catalog::domain::content_tree::{BookMeta, ContentEntry, PRIMARY_SOURCE_ID, SourceTree};

use super::*;

const LESSON_FILE: &str = "01-system-design/02-foundations/01-thinking-in-tradeoffs.md";
const LESSON_SOURCE: &str = "---\ntitle: Thinking in Tradeoffs\n---\n\nProse.\n";

struct FakeContent {
    tree: Vec<ContentEntry>,
    files: BTreeMap<String, String>,
}

impl ContentRepository for FakeContent {
    async fn content_version(&self) -> String {
        "v1".to_owned()
    }
    async fn load_sources(&self) -> Result<Vec<SourceTree>, ContentError> {
        Ok(vec![SourceTree {
            id: PRIMARY_SOURCE_ID.to_owned(),
            book_meta: None,
            category_meta: None,
            children: self.tree.clone(),
        }])
    }
    async fn read_lesson(&self, _source_id: &str, path: &str) -> Result<String, ContentError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| ContentError::NotFound(path.to_owned()))
    }
}

/// `01-system-design/` (a book) → `02-foundations/` (a chapter) → the lesson. The URL path drops
/// every order prefix, which is exactly what makes naive joining impossible.
fn source() -> FsLessonSource<FakeContent> {
    let mut files = BTreeMap::new();
    files.insert(LESSON_FILE.to_owned(), LESSON_SOURCE.to_owned());
    let tree = vec![ContentEntry::Dir {
        name: "01-system-design".to_owned(),
        book_meta: Some(BookMeta::default()),
        category_meta: None,
        children: vec![ContentEntry::Dir {
            name: "02-foundations".to_owned(),
            book_meta: None,
            category_meta: None,
            children: vec![ContentEntry::File {
                name: "01-thinking-in-tradeoffs.md".to_owned(),
                content: LESSON_SOURCE.to_owned(),
            }],
        }],
    }];
    FsLessonSource::new(FakeContent { tree, files })
}

fn path() -> Vec<String> {
    ["system-design", "foundations", "thinking-in-tradeoffs"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}

#[tokio::test]
async fn a_url_path_resolves_through_the_walker_to_the_numbered_file() {
    let file = source().file_for(&path()).await.unwrap().unwrap();
    assert_eq!(file.file_path, LESSON_FILE);
    assert_eq!(file.source, LESSON_SOURCE, "the frontmatter fence comes with it");
}

#[tokio::test]
async fn an_unknown_lesson_is_not_editable() {
    let missing = ["system-design", "foundations", "nope"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>();
    assert!(source().file_for(&missing).await.unwrap().is_none());
}

#[tokio::test]
async fn a_chapter_or_book_prefix_is_not_a_lesson() {
    for prefix in [
        vec!["system-design".to_owned()],
        vec!["system-design".to_owned(), "foundations".to_owned()],
    ] {
        assert!(source().file_for(&prefix).await.unwrap().is_none(), "{prefix:?}");
    }
}

#[tokio::test]
async fn a_traversal_attempt_never_reaches_the_filesystem() {
    // The slug check refuses before the tree is even loaded — `..` is not slug-like.
    for hostile in [
        vec!["..".to_owned(), "..".to_owned(), "etc".to_owned()],
        vec!["system-design".to_owned(), "..".to_owned(), "secrets".to_owned()],
        vec![String::new()],
        Vec::new(),
    ] {
        assert!(
            source().file_for(&hostile).await.unwrap().is_none(),
            "{hostile:?}"
        );
    }
}

/// Under `render-local-only` the study trees ARE in the catalog, so the resolver would find them
/// — and committing one would publish a gitignored file. Refused regardless of the feature.
#[tokio::test]
async fn local_only_content_is_never_editable() {
    for head in ["local-only-content", "local-only", "01-local-only-content"] {
        let file = format!("{head}/01-swiftly/01-intro.md");
        let mut files = BTreeMap::new();
        files.insert(file.clone(), LESSON_SOURCE.to_owned());
        let tree = vec![ContentEntry::Dir {
            name: head.to_owned(),
            book_meta: None,
            category_meta: None,
            children: vec![ContentEntry::Dir {
                name: "01-swiftly".to_owned(),
                book_meta: Some(BookMeta {
                    slug: Some("swiftly".to_owned()),
                    ..BookMeta::default()
                }),
                category_meta: None,
                children: vec![ContentEntry::File {
                    name: "01-intro.md".to_owned(),
                    content: LESSON_SOURCE.to_owned(),
                }],
            }],
        }];
        let source = FsLessonSource::new(FakeContent { tree, files });
        let path = ["swiftly".to_owned(), "intro".to_owned()];
        assert!(
            source.file_for(&path).await.unwrap().is_none(),
            "{head} must not be editable"
        );
    }
}
