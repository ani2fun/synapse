//! Tests for `FileSystemProblemTests` — hermetic, over an in-memory content repo with the
//! REAL numbered-dir shape (the walker map is the only correct path from slug to file).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use crate::catalog::domain::content_tree::{BookMeta, ContentEntry};

use super::*;

struct FakeContent {
    tree: Vec<ContentEntry>,
    files: BTreeMap<String, String>,
}

impl ContentRepository for FakeContent {
    async fn content_version(&self) -> String {
        "v1".to_owned()
    }
    async fn load_tree(&self) -> Result<Vec<ContentEntry>, ContentError> {
        Ok(self.tree.clone())
    }
    async fn read_lesson(&self, path: &str) -> Result<String, ContentError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| ContentError::NotFound(path.to_owned()))
    }
}

const SUITE: &str =
    r#"{"args":[{"id":"n","label":"N","type":"int"}],"cases":[{"args":{"n":"1"},"expected":"1"}]}"#;

fn content(files: BTreeMap<String, String>) -> FsProblemTests<FakeContent> {
    let tree = vec![ContentEntry::Dir {
        name: "01-learn".to_owned(),
        book_meta: None,
        category_meta: None,
        children: vec![ContentEntry::Dir {
            name: "02-dsa".to_owned(),
            book_meta: Some(BookMeta::default()),
            category_meta: None,
            children: vec![ContentEntry::File {
                name: "03-two-sum.md".to_owned(),
                content: files
                    .get("01-learn/02-dsa/03-two-sum.md")
                    .cloned()
                    .unwrap_or_default(),
            }],
        }],
    }];
    FsProblemTests::new(FakeContent { tree, files })
}

fn path() -> Vec<String> {
    vec!["learn".to_owned(), "dsa".to_owned(), "two-sum".to_owned()]
}

#[tokio::test]
async fn the_sidecar_is_found_through_the_walker_map() {
    let tests = content(BTreeMap::from([
        ("01-learn/02-dsa/03-two-sum.md".to_owned(), "prose".to_owned()),
        (
            "01-learn/02-dsa/03-two-sum.tests.json".to_owned(),
            SUITE.to_owned(),
        ),
    ]));
    let suite = tests.suite_for(&path()).await.unwrap().unwrap();
    assert_eq!(suite.cases.len(), 1);
}

#[tokio::test]
async fn no_sidecar_and_no_fence_is_not_a_problem() {
    let tests = content(BTreeMap::from([(
        "01-learn/02-dsa/03-two-sum.md".to_owned(),
        "just prose".to_owned(),
    )]));
    assert_eq!(tests.suite_for(&path()).await.unwrap(), None);
}

#[tokio::test]
async fn the_fence_is_the_fallback_but_the_sidecar_wins() {
    let with_fence = format!("prose\n```testcases\n{SUITE}\n```\nmore");
    let tests = content(BTreeMap::from([(
        "01-learn/02-dsa/03-two-sum.md".to_owned(),
        with_fence.clone(),
    )]));
    assert!(
        tests.suite_for(&path()).await.unwrap().is_some(),
        "fence is the suite"
    );

    let other_suite = SUITE.replace("\"1\"", "\"9\"");
    let tests = content(BTreeMap::from([
        ("01-learn/02-dsa/03-two-sum.md".to_owned(), with_fence),
        ("01-learn/02-dsa/03-two-sum.tests.json".to_owned(), other_suite),
    ]));
    let suite = tests.suite_for(&path()).await.unwrap().unwrap();
    assert_eq!(suite.cases[0].args["n"], "9", "the sidecar wins over the fence");
}

#[tokio::test]
async fn an_undecodable_suite_is_a_loud_invalid_suite() {
    for files in [
        BTreeMap::from([
            ("01-learn/02-dsa/03-two-sum.md".to_owned(), "prose".to_owned()),
            (
                "01-learn/02-dsa/03-two-sum.tests.json".to_owned(),
                "not json".to_owned(),
            ),
        ]),
        BTreeMap::from([(
            "01-learn/02-dsa/03-two-sum.md".to_owned(),
            "```testcases\nnot json\n```".to_owned(),
        )]),
    ] {
        let err = content(files).suite_for(&path()).await.unwrap_err();
        assert!(matches!(err, SubmissionError::InvalidSuite { .. }));
    }
}

#[tokio::test]
async fn an_unknown_lesson_is_not_a_problem() {
    let tests = content(BTreeMap::new());
    let missing = vec!["learn".to_owned(), "nope".to_owned()];
    assert_eq!(tests.suite_for(&missing).await.unwrap(), None);
}
