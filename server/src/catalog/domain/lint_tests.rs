//! The lint's job is the INVISIBLE failure — the one that renders fine and does the wrong thing.
//! Each test names the thing an author would never see by looking at the page.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::catalog::domain::content_tree::BookMeta;

fn file(name: &str, content: &str) -> ContentEntry {
    ContentEntry::File {
        name: name.to_owned(),
        content: content.to_owned(),
    }
}

fn dir(name: &str, children: Vec<ContentEntry>) -> ContentEntry {
    ContentEntry::Dir {
        name: name.to_owned(),
        book_meta: None,
        category_meta: None,
        children,
    }
}

fn book_source(slug: Option<&str>, children: Vec<ContentEntry>) -> SourceTree {
    SourceTree {
        id: "java-guide".to_owned(),
        book_meta: Some(BookMeta {
            title: Some("Java".to_owned()),
            slug: slug.map(ToOwned::to_owned),
            ..BookMeta::default()
        }),
        category_meta: None,
        children,
    }
}

fn lesson(body: &str) -> String {
    format!("---\ntitle: T\nsummary: s\n---\n{body}")
}

fn errors(findings: &[Finding]) -> Vec<&str> {
    findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| f.message.as_str())
        .collect()
}

// ── the book's own identity ───────────────────────────────────────────────────

#[test]
fn a_root_book_without_a_slug_is_an_error() {
    // The URL would silently become the repository's name and move every lesson.
    let findings = lint(
        &book_source(None, vec![file("01-a.md", &lesson("x"))]),
        &Sidecars::default(),
    );
    assert!(
        errors(&findings).iter().any(|m| m.contains("must set \"slug\"")),
        "{findings:?}"
    );
}

#[test]
fn a_root_book_with_a_good_slug_is_clean() {
    let findings = lint(
        &book_source(Some("java"), vec![file("01-a.md", &lesson("x"))]),
        &Sidecars::default(),
    );
    assert!(errors(&findings).is_empty(), "{findings:?}");
}

#[test]
fn a_slug_that_is_not_slug_like_is_an_error() {
    let findings = lint(
        &book_source(Some("Java Guide!"), vec![file("01-a.md", &lesson("x"))]),
        &Sidecars::default(),
    );
    assert!(
        errors(&findings).iter().any(|m| m.contains("not slug-like")),
        "{findings:?}"
    );
}

// ── lessons ───────────────────────────────────────────────────────────────────

#[test]
fn a_lesson_without_a_summary_warns_rather_than_fails() {
    // It renders; the catalog card and the meta description are just empty.
    let findings = lint(
        &book_source(Some("java"), vec![file("01-a.md", "---\ntitle: T\n---\nbody")]),
        &Sidecars::default(),
    );
    assert!(errors(&findings).is_empty(), "{findings:?}");
    assert!(
        findings.iter().any(|f| f.message.contains("no `summary:`")),
        "{findings:?}"
    );
}

#[test]
fn a_lesson_with_no_frontmatter_fence_is_an_error() {
    let findings = lint(
        &book_source(Some("java"), vec![file("01-a.md", "# Just a heading")]),
        &Sidecars::default(),
    );
    assert!(
        errors(&findings)
            .iter()
            .any(|m| m.contains("no frontmatter fence")),
        "{findings:?}"
    );
}

#[test]
fn repo_furniture_is_not_linted_as_a_lesson() {
    // README.md sits INSIDE the book when the root is the book, but it never renders.
    let findings = lint(
        &book_source(Some("java"), vec![file("README.md", "how to contribute")]),
        &Sidecars::default(),
    );
    assert!(errors(&findings).is_empty(), "{findings:?}");
}

// ── problems ──────────────────────────────────────────────────────────────────

#[test]
fn a_problem_with_no_suite_at_all_is_an_error() {
    // The workbench renders and Submit has nothing to judge.
    let source = book_source(
        Some("dsa"),
        vec![dir(
            "01-two-sum",
            vec![file(
                "two-sum.md",
                "---\ntitle: T\nsummary: s\nkind: problem\n---\nbody",
            )],
        )],
    );
    let findings = lint(&source, &Sidecars::default());
    assert!(
        errors(&findings).iter().any(|m| m.contains("nothing to judge")),
        "{findings:?}"
    );
}

#[test]
fn a_problem_with_a_tests_json_sidecar_is_clean() {
    let source = book_source(
        Some("dsa"),
        vec![dir(
            "01-two-sum",
            vec![file(
                "two-sum.md",
                "---\ntitle: T\nsummary: s\nkind: problem\n---\nbody",
            )],
        )],
    );
    let sidecars = Sidecars {
        test_suites: vec!["01-two-sum/two-sum.tests.json".to_owned()],
        ..Sidecars::default()
    };
    assert!(errors(&lint(&source, &sidecars)).is_empty());
}

#[test]
fn a_problem_with_only_a_testcases_fence_is_clean() {
    let body = "---\ntitle: T\nsummary: s\nkind: problem\n---\n```testcases\n[]\n```";
    let source = book_source(Some("dsa"), vec![file("01-p.md", body)]);
    assert!(errors(&lint(&source, &Sidecars::default())).is_empty());
}

#[test]
fn a_suite_whose_lesson_was_renamed_away_is_flagged() {
    // The commonest way to break a problem: rename the lesson, leave the sidecars behind.
    let source = book_source(Some("dsa"), vec![file("01-new-name.md", &lesson("x"))]);
    let sidecars = Sidecars {
        test_suites: vec!["01-old-name.tests.json".to_owned()],
        ..Sidecars::default()
    };
    let findings = lint(&source, &sidecars);
    assert!(
        findings.iter().any(|f| f.message.contains("never read")),
        "{findings:?}"
    );
}

// ── quiz fences ───────────────────────────────────────────────────────────────

#[test]
fn a_quiz_whose_answer_matches_no_option_is_an_error() {
    // It renders as a card that can never be got right.
    let body = "---\ntitle: T\nsummary: s\n---\n```quiz\n{\"prompt\":\"p\",\"options\":[\"A\",\"B\"],\"answer\":\"C\"}\n```";
    let findings = lint(
        &book_source(Some("x"), vec![file("01-a.md", body)]),
        &Sidecars::default(),
    );
    assert!(
        errors(&findings).iter().any(|m| m.contains("matches no option")),
        "{findings:?}"
    );
}

#[test]
fn a_well_formed_quiz_is_clean() {
    let body = "---\ntitle: T\nsummary: s\n---\n```quiz\n{\"prompt\":\"p\",\"options\":[\"A\",\"B\"],\"answer\":\"B\"}\n```";
    assert!(
        errors(&lint(
            &book_source(Some("x"), vec![file("01-a.md", body)]),
            &Sidecars::default()
        ))
        .is_empty()
    );
}

#[test]
fn a_quiz_that_is_not_json_is_an_error() {
    let body = "---\ntitle: T\nsummary: s\n---\n```quiz\nnot json\n```";
    let findings = lint(
        &book_source(Some("x"), vec![file("01-a.md", body)]),
        &Sidecars::default(),
    );
    assert!(
        errors(&findings).iter().any(|m| m.contains("not valid JSON")),
        "{findings:?}"
    );
}

#[test]
fn an_editorial_beside_a_lesson_that_is_not_a_problem_is_never_rendered() {
    // The 19-lesson shape found in dsa-guide on the validator's first run: the editorial is
    // written, the lesson has no frontmatter at all, so it renders as prose and the editorial
    // is silently ignored.
    let source = book_source(
        Some("dsa"),
        vec![
            file("01-p.md", "# Palindrome Check\n\nprose, no frontmatter"),
            file("01-p.editorial.md", "the walkthrough"),
        ],
    );
    let findings = lint(&source, &Sidecars::default());
    assert!(
        findings
            .iter()
            .any(|f| f.path == "01-p.editorial.md" && f.message.contains("not `kind: problem`")),
        "{findings:?}"
    );
}

#[test]
fn an_editorial_beside_a_real_problem_is_clean() {
    let source = book_source(
        Some("dsa"),
        vec![
            file(
                "01-p.md",
                "---\ntitle: T\nsummary: s\nkind: problem\n---\n```testcases\n[]\n```",
            ),
            file("01-p.editorial.md", "the walkthrough"),
        ],
    );
    let findings = lint(&source, &Sidecars::default());
    assert!(findings.is_empty(), "{findings:?}");
}
