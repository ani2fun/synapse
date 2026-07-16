//! Oracle: `SubmitSolutionSpec` — the judge over in-memory fakes. `judge_and_complete` is
//! driven DIRECTLY (pub(crate)) for determinism; `submit`'s detached task is fire-and-forget.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// The scripted helpers mirror the runner's Result shape on purpose; sort_by mirrors the repo.
#![allow(clippy::unnecessary_wraps, clippy::stable_sort_primitive)]

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Mutex;

use synapse_shared::execution::{ArgSpec, RunResult, RunStatus, TestCase};

use super::*;
use crate::execution::domain::Language;

// ── fakes ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct FakeRepo {
    rows: Mutex<HashMap<Uuid, Submission>>,
    state_log: Mutex<Vec<String>>,
}

impl SubmissionRepository for FakeRepo {
    async fn save(&self, s: &Submission) -> Result<(), SubmissionError> {
        self.state_log.lock().unwrap().push("save:pending".to_owned());
        self.rows.lock().unwrap().insert(s.id.0, s.clone());
        Ok(())
    }
    async fn update(&self, s: &Submission) -> Result<(), SubmissionError> {
        let label = match &s.state {
            SubmissionState::Pending => "pending",
            SubmissionState::Judging => "judging",
            SubmissionState::Completed { .. } => "completed",
        };
        self.state_log.lock().unwrap().push(format!("update:{label}"));
        self.rows.lock().unwrap().insert(s.id.0, s.clone());
        Ok(())
    }
    async fn get(&self, id: SubmissionId) -> Result<Option<Submission>, SubmissionError> {
        Ok(self.rows.lock().unwrap().get(&id.0).cloned())
    }
    async fn list_for(
        &self,
        lesson_path: &[String],
        by_user: Option<&str>,
    ) -> Result<Vec<Submission>, SubmissionError> {
        let mut rows: Vec<Submission> = self
            .rows
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.lesson_path == lesson_path)
            .filter(|s| by_user.is_none_or(|u| s.user_id.as_deref() == Some(u)))
            .cloned()
            .collect();
        rows.sort_by_key(|s| std::cmp::Reverse(s.created_at));
        Ok(rows)
    }
}

struct FakeTests(Option<TestSpec>);

impl ProblemTests for FakeTests {
    async fn suite_for(&self, _path: &[String]) -> Result<Option<TestSpec>, SubmissionError> {
        Ok(self.0.clone())
    }
}

/// Scripted runner: pops one canned reply per call, recording every stdin it saw. The `Arc`d
/// interior lets the test keep a probe handle after the runner moves into the service.
#[derive(Default, Clone)]
struct ScriptedRunner {
    replies: Arc<Mutex<Vec<Result<RunResult, ExecutionError>>>>,
    stdins: Arc<Mutex<Vec<Option<String>>>>,
}

impl crate::execution::application::CodeRunner for ScriptedRunner {
    async fn run(
        &self,
        _language: Language,
        _source: &str,
        stdin: Option<&str>,
    ) -> Result<RunResult, ExecutionError> {
        self.stdins.lock().unwrap().push(stdin.map(str::to_owned));
        self.replies.lock().unwrap().remove(0)
    }
}

fn ok_run(stdout: &str) -> Result<RunResult, ExecutionError> {
    Ok(RunResult {
        status: RunStatus::Accepted,
        stdout: stdout.to_owned(),
        stderr: String::new(),
        compile_output: String::new(),
        time_seconds: None,
        memory_kb: None,
    })
}

fn crashed_run() -> Result<RunResult, ExecutionError> {
    Ok(RunResult {
        status: RunStatus::RuntimeError,
        stdout: String::new(),
        stderr: "boom".to_owned(),
        compile_output: String::new(),
        time_seconds: None,
        memory_kb: None,
    })
}

fn spec(expected: &[Option<&str>]) -> TestSpec {
    TestSpec {
        args: vec![ArgSpec {
            id: "n".to_owned(),
            label: "N".to_owned(),
            tpe: "int".to_owned(),
            placeholder: None,
        }],
        cases: expected
            .iter()
            .enumerate()
            .map(|(i, e)| TestCase {
                args: BTreeMap::from([("n".to_owned(), i.to_string())]),
                expected: e.map(str::to_owned),
            })
            .collect(),
    }
}

fn service(
    suite: Option<TestSpec>,
    replies: Vec<Result<RunResult, ExecutionError>>,
) -> (
    SubmitSolution<FakeRepo, FakeTests, ScriptedRunner>,
    ScriptedRunner,
) {
    let runner = ScriptedRunner {
        replies: Arc::new(Mutex::new(replies)),
        ..ScriptedRunner::default()
    };
    let probe = runner.clone();
    let svc = SubmitSolution::new(
        Arc::new(FakeRepo::default()),
        Arc::new(FakeTests(suite)),
        Arc::new(RunCodeService::new(runner)),
    );
    (svc, probe)
}

fn path() -> Vec<String> {
    vec!["dsa".to_owned(), "two-sum".to_owned()]
}

// ── behaviors ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_lesson_without_a_suite_is_not_a_problem_and_stores_nothing() {
    let (svc, _) = service(None, vec![]);
    let err = svc.submit(path(), "python".into(), "x".into()).await.unwrap_err();
    assert!(matches!(err, SubmissionError::NotAProblem(_)));
    assert!(svc.repo.rows.lock().unwrap().is_empty());
}

#[tokio::test]
async fn submit_persists_pending_and_anonymous_and_returns_the_id() {
    let (svc, _) = service(Some(spec(&[Some("0")])), vec![ok_run("0")]);
    let id = svc
        .submit(path(), "python".into(), "print(n)".into())
        .await
        .unwrap();
    let stored = svc.repo.get(id).await.unwrap().unwrap();
    assert_eq!(stored.user_id, None, "the anonymous seam");
    assert_eq!(stored.lesson_path, path());
}

#[tokio::test]
async fn an_all_pass_suite_is_accepted_in_authored_order_with_the_stdin_shape() {
    let (svc, probe) = service(None, vec![ok_run("0"), ok_run("1"), ok_run("2")]);
    let outcome = svc
        .judge(&spec(&[Some("0"), Some("1"), Some("2")]), "python", "src")
        .await;
    assert_eq!(outcome, SuiteOutcome::Accepted { total: 3 });
    let stdins = probe.stdins.lock().unwrap().clone();
    assert_eq!(
        stdins,
        vec![
            Some("0\n".to_owned()),
            Some("1\n".to_owned()),
            Some("2\n".to_owned())
        ]
    );
}

#[tokio::test]
async fn judging_stops_at_the_first_failure() {
    let (svc, probe) = service(None, vec![ok_run("0"), ok_run("wrong"), ok_run("never-run")]);
    let outcome = svc
        .judge(&spec(&[Some("0"), Some("1"), Some("2")]), "python", "src")
        .await;
    let SuiteOutcome::Rejected {
        passed,
        total,
        first_failure,
    } = outcome
    else {
        panic!("expected a rejection");
    };
    assert_eq!((passed, total), (1, 3));
    assert_eq!(first_failure.index, 1);
    assert_eq!(first_failure.stdout, "wrong");
    assert_eq!(probe.stdins.lock().unwrap().len(), 2, "the third case never runs");
}

#[tokio::test]
async fn a_crash_is_a_rejection_carrying_the_crash_status() {
    let (svc, _) = service(None, vec![crashed_run()]);
    let outcome = svc.judge(&spec(&[Some("0")]), "python", "src").await;
    let SuiteOutcome::Rejected { first_failure, .. } = outcome else {
        panic!("expected rejection")
    };
    assert_eq!(first_failure.status, RunStatus::RuntimeError);
    assert_eq!(first_failure.stderr, "boom");
}

#[tokio::test]
async fn machinery_failure_mid_suite_is_judge_failed_with_passes_so_far() {
    let (svc, _) = service(
        None,
        vec![
            ok_run("0"),
            Err(ExecutionError::BackendUnavailable("down".into())),
        ],
    );
    let outcome = svc
        .judge(&spec(&[Some("0"), Some("1"), Some("2")]), "python", "src")
        .await;
    assert_eq!(
        outcome,
        SuiteOutcome::JudgeFailed {
            passed: 1,
            total: 3,
            detail: "execution backend unavailable".to_owned()
        }
    );
}

#[tokio::test]
async fn a_clean_run_with_no_expected_output_counts_as_a_pass() {
    let (svc, _) = service(None, vec![ok_run("whatever")]);
    let outcome = svc.judge(&spec(&[None]), "python", "src").await;
    assert_eq!(outcome, SuiteOutcome::Accepted { total: 1 });
}

#[tokio::test]
async fn judge_and_complete_walks_judging_then_completed_and_never_sticks() {
    let (svc, _) = service(None, vec![ok_run("0")]);
    let submission = Submission {
        id: SubmissionId(Uuid::new_v4()),
        lesson_path: path(),
        language: "python".into(),
        source: "src".into(),
        user_id: None,
        created_at: Utc::now(),
        state: SubmissionState::Pending,
    };
    svc.repo.save(&submission).await.unwrap();
    svc.judge_and_complete(submission.clone(), spec(&[Some("0")]))
        .await;
    let log = svc.repo.state_log.lock().unwrap().clone();
    assert_eq!(log, vec!["save:pending", "update:judging", "update:completed"]);
    let stored = svc.repo.get(submission.id).await.unwrap().unwrap();
    assert!(stored.state.is_completed());
}

#[tokio::test]
async fn get_unknown_is_unknown_submission() {
    let (svc, _) = service(Some(spec(&[Some("0")])), vec![]);
    let err = svc.get(SubmissionId(Uuid::new_v4())).await.unwrap_err();
    assert!(matches!(err, SubmissionError::UnknownSubmission(_)));
}
