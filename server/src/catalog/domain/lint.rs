//! What a content repository gets wrong, found before it is pushed.
//!
//! Every rule here exists because the failure it catches is INVISIBLE in the rendered page. A
//! lesson with no summary renders fine and ships an empty meta description; a problem with no test
//! suite renders a workbench with nothing to judge; an orphaned `.tests.json` left behind by a
//! rename is simply never read again. None of that shows up by looking at the site, which is why
//! "the running app is the final render authority" was never enough on its own.
//!
//! Pure: the caller supplies the tree and the list of non-markdown files beside it, and gets
//! findings back. The same walker the server runs decides what is a lesson, so this cannot drift
//! from what actually renders.

use std::collections::BTreeSet;

use crate::catalog::domain::content_tree::{ContentEntry, SourceTree};
use crate::catalog::domain::frontmatter;
use crate::catalog::domain::walker;

/// An error means the repository will not render as intended; a warning means it will, but
/// something an author meant is being ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    /// The file the finding is about, source-root-relative.
    pub path: String,
    pub message: String,
}

impl Finding {
    fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
        }
    }

    fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Everything beside the markdown that the walker never sees but the server still reads.
#[derive(Debug, Clone, Default)]
pub struct Sidecars {
    /// Source-root-relative paths of every `*.tests.json`.
    pub test_suites: Vec<String>,
    /// Source-root-relative paths of every `*.c4`.
    pub c4_files: Vec<String>,
}

/// Lint one content source.
#[must_use]
pub fn lint(source: &SourceTree, sidecars: &Sidecars) -> Vec<Finding> {
    let mut findings = Vec::new();

    if source.book_meta.is_some() {
        check_root_book(source, &mut findings);
    }

    let mut seen = Seen::default();
    walk(&source.children, &[], &mut seen, sidecars, &mut findings);
    check_orphans(&seen, sidecars, &mut findings);
    findings.sort_by(|a, b| (a.severity, &a.path).cmp(&(b.severity, &b.path)));
    findings
}

/// A repository whose root IS the book has no directory to derive a slug from, so `book.json` has
/// to carry one — otherwise the URL silently becomes the repository's name.
fn check_root_book(source: &SourceTree, findings: &mut Vec<Finding>) {
    let Some(meta) = source.book_meta.as_ref() else {
        return;
    };
    match meta.slug.as_deref() {
        None | Some("") => findings.push(Finding::error(
            "book.json",
            "a repository whose root is the book must set \"slug\" — without it the URL falls back \
             to the source id, which moves every lesson",
        )),
        Some(slug) if !walker::slug_like(slug) => findings.push(Finding::error(
            "book.json",
            format!("\"slug\": \"{slug}\" is not slug-like (alphanumeric, '-' or '_')"),
        )),
        Some(_) => {}
    }
    if meta.title.is_none() {
        findings.push(Finding::warning(
            "book.json",
            "no \"title\" — the catalog will humanise the source id",
        ));
    }
}

/// What the walk noticed, for the cross-file checks that can only run once it is over.
#[derive(Default)]
struct Seen {
    lessons: BTreeSet<String>,
    /// Lessons carrying `kind: problem` — the only ones whose sidecars are ever read.
    problems: BTreeSet<String>,
    /// `*.editorial.md` files, which the walker excludes from the catalog by suffix.
    editorials: BTreeSet<String>,
}

fn walk(
    entries: &[ContentEntry],
    dirs: &[String],
    seen: &mut Seen,
    sidecars: &Sidecars,
    findings: &mut Vec<Finding>,
) {
    for entry in entries {
        match entry {
            // A directory the walker skips carries no lessons, so linting inside one reports
            // problems about files that never render — `_c4-docs/` click-docs most of all.
            ContentEntry::Dir { name, children, .. } if walker::includes_as_content(name) => {
                let mut inner = dirs.to_vec();
                inner.push(name.clone());
                walk(children, &inner, seen, sidecars, findings);
            }
            ContentEntry::Dir { .. } => {}
            ContentEntry::File { name, content } => {
                let mut path = dirs.to_vec();
                path.push(name.clone());
                let path = path.join("/");
                if name.ends_with(".editorial.md") {
                    seen.editorials.insert(path);
                } else if walker::is_lesson_file(name) {
                    if frontmatter::extract_kind(content).as_deref() == Some("problem") {
                        seen.problems.insert(path.clone());
                    }
                    seen.lessons.insert(path.clone());
                    check_lesson(&path, content, sidecars, findings);
                }
            }
        }
    }
}

fn check_lesson(path: &str, content: &str, sidecars: &Sidecars, findings: &mut Vec<Finding>) {
    if frontmatter::extract_summary(content).is_none() {
        findings.push(Finding::warning(
            path,
            "no `summary:` — the catalog card and the page description will be empty",
        ));
    }
    if !content.trim_start().starts_with("---") {
        findings.push(Finding::error(
            path,
            "no frontmatter fence — `title:` and `summary:` cannot be read",
        ));
    }
    check_quiz_fences(path, content, findings);

    if frontmatter::extract_kind(content).as_deref() == Some("problem") {
        check_problem(path, content, sidecars, findings);
    }
}

/// A workbench with nothing to judge renders perfectly and does nothing.
fn check_problem(path: &str, content: &str, sidecars: &Sidecars, findings: &mut Vec<Finding>) {
    let suite = format!("{}.tests.json", stem(path));
    let has_sidecar = sidecars.test_suites.iter().any(|s| s == &suite);
    let has_fence = content.contains("```testcases");
    if !has_sidecar && !has_fence {
        findings.push(Finding::error(
            path,
            format!(
                "`kind: problem` with neither {suite} nor a ```testcases fence — Submit has nothing to judge"
            ),
        ));
    }
}

/// `answer` must equal one option EXACTLY, or the card renders and can never be got right.
fn check_quiz_fences(path: &str, content: &str, findings: &mut Vec<Finding>) {
    for (index, block) in fenced_blocks(content, "quiz").enumerate() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(block) else {
            findings.push(Finding::error(
                path,
                format!("quiz block {} is not valid JSON", index + 1),
            ));
            continue;
        };
        let answer = value.get("answer").and_then(serde_json::Value::as_str);
        let options: Vec<&str> = value
            .get("options")
            .and_then(serde_json::Value::as_array)
            .map(|a| a.iter().filter_map(serde_json::Value::as_str).collect())
            .unwrap_or_default();
        match answer {
            Some(answer) if options.contains(&answer) => {}
            Some(answer) => findings.push(Finding::error(
                path,
                format!(
                    "quiz block {}: \"answer\": \"{answer}\" matches no option exactly",
                    index + 1
                ),
            )),
            None => findings.push(Finding::error(
                path,
                format!("quiz block {}: no \"answer\"", index + 1),
            )),
        }
    }
}

/// Sidecars nothing will ever read. Two ways that happens, and both are silent: the lesson was
/// renamed and its sidecars left behind, or the lesson is there but was never marked
/// `kind: problem` — in which case it renders as prose and its editorial and suite are ignored.
fn check_orphans(seen: &Seen, sidecars: &Sidecars, findings: &mut Vec<Finding>) {
    let mut check = |sidecar: &String, lesson: String| {
        if !seen.lessons.contains(&lesson) {
            findings.push(Finding::warning(
                sidecar,
                format!("no lesson at {lesson} — this sidecar is never read"),
            ));
        } else if !seen.problems.contains(&lesson) {
            findings.push(Finding::warning(
                sidecar,
                format!("{lesson} is not `kind: problem`, so this sidecar is never rendered"),
            ));
        }
    };
    for suite in &sidecars.test_suites {
        check(suite, format!("{}.md", suite.trim_end_matches(".tests.json")));
    }
    for editorial in &seen.editorials {
        check(
            editorial,
            format!("{}.md", editorial.trim_end_matches(".editorial.md")),
        );
    }
}

fn stem(path: &str) -> &str {
    path.strip_suffix(".md").unwrap_or(path)
}

/// The bodies of every ```` ```<tag> ```` fence, in order.
fn fenced_blocks<'a>(content: &'a str, tag: &'a str) -> impl Iterator<Item = &'a str> {
    let open = format!("```{tag}");
    let mut rest = content;
    std::iter::from_fn(move || {
        loop {
            let start = rest.find(&open)?;
            let after = &rest[start + open.len()..];
            // `quiz` must be the whole info string — not a prefix of `quizzes`.
            let body_start = after.find('\n')?;
            let info = after[..body_start].trim();
            let end = after[body_start + 1..].find("```")?;
            let body = &after[body_start + 1..body_start + 1 + end];
            rest = &after[body_start + 1 + end + 3..];
            if info.is_empty() {
                return Some(body);
            }
        }
    })
}

#[cfg(test)]
#[path = "lint_tests.rs"]
mod tests;
