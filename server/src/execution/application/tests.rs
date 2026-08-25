//! Validation tests for `RunCodeService`, driven over a recording fake runner.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use synapse_shared::execution::RunStatus;

use crate::execution::domain::GO_JUDGE_LIMITS;

use super::*;

#[derive(Default)]
struct FakeRunner {
    calls: Mutex<Vec<(Language, String, Option<String>)>>,
    fail_with: Option<ExecutionError>,
}

impl CodeRunner for FakeRunner {
    async fn run(
        &self,
        language: Language,
        source: &str,
        stdin: Option<&str>,
    ) -> Result<RunResult, ExecutionError> {
        self.calls
            .lock()
            .unwrap()
            .push((language, source.to_owned(), stdin.map(str::to_owned)));
        if let Some(error) = &self.fail_with {
            return Err(error.clone());
        }
        Ok(RunResult {
            status: RunStatus::Accepted,
            stdout: "ok".to_owned(),
            stderr: String::new(),
            compile_output: String::new(),
            time_seconds: None,
            memory_kb: None,
        })
    }
}

fn request(language: &str, source: &str, stdin: Option<&str>) -> RunRequest {
    RunRequest {
        language: language.to_owned(),
        source: source.to_owned(),
        stdin: stdin.map(str::to_owned),
    }
}

#[tokio::test]
async fn unknown_languages_never_reach_the_runner() {
    let service = RunCodeService::new(FakeRunner::default());
    let err = service.run(&request("cobol", "x", None)).await.unwrap_err();
    assert_eq!(err, ExecutionError::UnknownLanguage("cobol".to_owned()));
    assert!(service.runner.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn oversized_payloads_are_rejected_before_running() {
    let service = RunCodeService::new(FakeRunner::default());
    let big_source = "x".repeat(GO_JUDGE_LIMITS.max_source_bytes + 1);
    assert!(matches!(
        service.run(&request("py", &big_source, None)).await.unwrap_err(),
        ExecutionError::PayloadTooLarge { field: "Source", .. }
    ));
    let big_stdin = "x".repeat(GO_JUDGE_LIMITS.max_stdin_bytes + 1);
    assert!(matches!(
        service
            .run(&request("py", "print(1)", Some(&big_stdin)))
            .await
            .unwrap_err(),
        ExecutionError::PayloadTooLarge {
            field: "Standard input",
            ..
        }
    ));
    assert!(service.runner.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_caps_are_inclusive() {
    let service = RunCodeService::new(FakeRunner::default());
    let at_limit = "x".repeat(GO_JUDGE_LIMITS.max_source_bytes);
    assert!(service.run(&request("py", &at_limit, None)).await.is_ok());
}

#[tokio::test]
async fn the_resolved_language_and_payload_reach_the_runner() {
    let service = RunCodeService::new(FakeRunner::default());
    service
        .run(&request("  PY ", "print(1)", Some("42")))
        .await
        .unwrap();
    let calls = service.runner.calls.lock().unwrap();
    assert_eq!(
        calls.as_slice(),
        &[(Language::Python, "print(1)".to_owned(), Some("42".to_owned()))]
    );
}

#[tokio::test]
async fn backend_failures_propagate() {
    let service = RunCodeService::new(FakeRunner {
        fail_with: Some(ExecutionError::BackendFailed("boom".to_owned())),
        ..FakeRunner::default()
    });
    assert_eq!(
        service.run(&request("py", "x", None)).await.unwrap_err(),
        ExecutionError::BackendFailed("boom".to_owned())
    );
}
