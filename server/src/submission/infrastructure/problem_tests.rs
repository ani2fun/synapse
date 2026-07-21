//! Where a problem's hidden suite lives (`FileSystemProblemTests`) — resolved THROUGH
//! the catalog walker's lesson-file map (naive path joining is impossible: real folders carry
//! `NN-` order prefixes). Two tiers: the `.tests.json` sidecar is authoritative; a trailing
//! testcases fence in the lesson itself is the fallback (the fence-only problems fix). Absent
//! both → not a problem. A suite that WON'T DECODE is a loud `InvalidSuite` — authoring
//! mistakes must surface, never silently degrade. Re-read per lookup → hot reload.

use std::sync::LazyLock;

use regex::Regex;
use synapse_shared::execution::TestSpec;

use crate::catalog::application::{ContentError, ContentRepository};
use crate::catalog::domain::{resolver, walker};
use crate::submission::application::{ProblemTests, SubmissionError};

static TESTCASES_FENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)```testcases\s*\n(.*?)```").unwrap_or_else(|e| unreachable!("static regex: {e}"))
});

pub struct FsProblemTests<R> {
    repo: R,
}

impl<R> FsProblemTests<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: ContentRepository> ProblemTests for FsProblemTests<R> {
    async fn suite_for(&self, lesson_path: &[String]) -> Result<Option<TestSpec>, SubmissionError> {
        let joined = lesson_path.join("/");
        let tree = self.repo.load_tree().await.map_err(|e| content_failed(&e))?;
        let walk = walker::walk(&tree)
            .map_err(|error| SubmissionError::StoreFailed(format!("catalog index invalid: {error}")))?;
        let Some((book, in_book_path, _)) = resolver::resolve_lesson(&walk.catalog, lesson_path) else {
            return Ok(None);
        };
        let Some(file) = walk
            .lesson_files
            .get(&book.slug)
            .and_then(|files| files.get(&in_book_path))
        else {
            return Ok(None);
        };

        // Tier 1 — the sidecar is authoritative.
        if let Some(stem) = file.strip_suffix(".md") {
            match self.repo.read_lesson(&format!("{stem}.tests.json")).await {
                Ok(raw) => return decode(&raw, &joined).map(Some),
                Err(ContentError::NotFound(_)) => {}
                Err(error) => return Err(content_failed(&error)),
            }
        }

        // Tier 2 — a testcases fence inside the lesson itself.
        let markdown = match self.repo.read_lesson(file).await {
            Ok(markdown) => markdown,
            Err(ContentError::NotFound(_)) => return Ok(None),
            Err(error) => return Err(content_failed(&error)),
        };
        match TESTCASES_FENCE.captures(&markdown).and_then(|c| c.get(1)) {
            Some(fence) if !fence.as_str().trim().is_empty() => decode(fence.as_str(), &joined).map(Some),
            _ => Ok(None),
        }
    }
}

fn decode(raw: &str, path: &str) -> Result<TestSpec, SubmissionError> {
    serde_json::from_str(raw).map_err(|error| SubmissionError::InvalidSuite {
        path: path.to_owned(),
        detail: error.to_string(),
    })
}

fn content_failed(error: &ContentError) -> SubmissionError {
    SubmissionError::StoreFailed(format!("content access failed: {error}"))
}

#[cfg(test)]
#[path = "problem_tests_tests.rs"]
mod tests;
