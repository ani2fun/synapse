//! The submission use cases (oracle: `SubmitSolution` + ports, step 14 scope). Anonymous-first:
//! `user_id` stays `None` until the identity step fills it; the ports already carry the seams
//! (`by_user`, owner checks) so identity slots in without reshaping the aggregate.

use std::sync::Arc;

use chrono::Utc;
use synapse_shared::execution::{RunRequest, TestSpec, Verdict, judge, stdin_for};
use uuid::Uuid;

use crate::execution::application::{CodeRunner, ExecutionError, RunCodeService};
use crate::submission::domain::{FailedCase, Submission, SubmissionId, SubmissionState, SuiteOutcome};

/// The context's error. HTTP mapping (next step): `NotAProblem`/`UnknownSubmission`→404,
/// `InvalidSuite`/`StoreFailed`→500. A program failing its cases is NOT an error — it is a
/// completed submission with a `Rejected` outcome.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubmissionError {
    #[error("'{0}' has no hidden suite — not a problem")]
    NotAProblem(String),
    #[error("the authored suite for '{path}' will not decode: {detail}")]
    InvalidSuite { path: String, detail: String },
    #[error("no submission '{0}'")]
    UnknownSubmission(String),
    #[error("submission store failed: {0}")]
    StoreFailed(String),
}

/// The submissions store (oracle: `SubmissionRepository`). Owner checks are the APPLICATION's
/// job — the port just persists.
pub trait SubmissionRepository: Send + Sync {
    fn save(&self, submission: &Submission) -> impl Future<Output = Result<(), SubmissionError>> + Send;
    fn update(&self, submission: &Submission) -> impl Future<Output = Result<(), SubmissionError>> + Send;
    fn get(
        &self,
        id: SubmissionId,
    ) -> impl Future<Output = Result<Option<Submission>, SubmissionError>> + Send;
    /// Newest first; `by_user` narrows to the owner (the identity step's "mine" scoping).
    fn list_for(
        &self,
        lesson_path: &[String],
        by_user: Option<&str>,
    ) -> impl Future<Output = Result<Vec<Submission>, SubmissionError>> + Send;
}

/// Where a problem's hidden suite comes from (oracle: `ProblemTests`) — `None` = not a problem.
pub trait ProblemTests: Send + Sync {
    fn suite_for(
        &self,
        lesson_path: &[String],
    ) -> impl Future<Output = Result<Option<TestSpec>, SubmissionError>> + Send;
}

/// Submit → 202 → background judge → poll. Cloning shares the same adapters (`Arc`s), which is
/// what lets the judge run as a DETACHED task outliving the request.
pub struct SubmitSolution<Repo, Tests, R: CodeRunner> {
    repo: Arc<Repo>,
    tests: Arc<Tests>,
    runner: Arc<RunCodeService<R>>,
}

impl<Repo, Tests, R: CodeRunner> Clone for SubmitSolution<Repo, Tests, R> {
    fn clone(&self) -> Self {
        Self {
            repo: Arc::clone(&self.repo),
            tests: Arc::clone(&self.tests),
            runner: Arc::clone(&self.runner),
        }
    }
}

impl<Repo, Tests, R> SubmitSolution<Repo, Tests, R>
where
    Repo: SubmissionRepository + Send + Sync + 'static,
    Tests: ProblemTests + Send + Sync + 'static,
    R: CodeRunner + Send + Sync + 'static,
{
    pub fn new(repo: Arc<Repo>, tests: Arc<Tests>, runner: Arc<RunCodeService<R>>) -> Self {
        Self { repo, tests, runner }
    }

    /// Store `Pending`, fire the judge as a detached task, answer immediately (the 202).
    pub async fn submit(
        &self,
        lesson_path: Vec<String>,
        language: String,
        source: String,
    ) -> Result<SubmissionId, SubmissionError> {
        let joined = lesson_path.join("/");
        let spec = self
            .tests
            .suite_for(&lesson_path)
            .await?
            .ok_or(SubmissionError::NotAProblem(joined))?;
        let submission = Submission {
            id: SubmissionId(Uuid::new_v4()),
            lesson_path,
            language,
            source,
            user_id: None, // the anonymous seam — identity fills it
            created_at: Utc::now(),
            state: SubmissionState::Pending,
        };
        let id = submission.id;
        self.repo.save(&submission).await?;
        tracing::info!(%id, "submission stored — judging in background");
        let this = self.clone();
        tokio::spawn(async move { this.judge_and_complete(submission, spec).await });
        Ok(id)
    }

    pub async fn get(&self, id: SubmissionId) -> Result<Submission, SubmissionError> {
        self.repo
            .get(id)
            .await?
            .ok_or_else(|| SubmissionError::UnknownSubmission(id.to_string()))
    }

    pub async fn list_for(
        &self,
        lesson_path: &[String],
        by_user: Option<&str>,
    ) -> Result<Vec<Submission>, SubmissionError> {
        self.repo.list_for(lesson_path, by_user).await
    }

    /// Judging → outcome → completed. INFALLIBLE with a backstop: any pipeline failure records
    /// `JudgeFailed` best-effort so a row is never left stuck on Judging.
    pub(crate) async fn judge_and_complete(&self, submission: Submission, spec: TestSpec) {
        let total = spec.cases.len();
        let outcome = match self.repo.update(&submission.judging()).await {
            Ok(()) => self.judge(&spec, &submission.language, &submission.source).await,
            Err(error) => SuiteOutcome::JudgeFailed {
                passed: 0,
                total,
                detail: error.to_string(),
            },
        };
        if let Err(error) = self.repo.update(&submission.completed(outcome, Utc::now())).await {
            tracing::warn!(id = %submission.id, %error, "could not record the outcome");
        }
    }

    /// Run in AUTHORED ORDER, stop at the first failure. Never fails — machinery trouble is the
    /// `JudgeFailed` outcome.
    pub(crate) async fn judge(&self, spec: &TestSpec, language: &str, source: &str) -> SuiteOutcome {
        let total = spec.cases.len();
        let mut passed = 0;
        for (index, case) in spec.cases.iter().enumerate() {
            let request = RunRequest {
                language: language.to_owned(),
                source: source.to_owned(),
                stdin: Some(stdin_for(&spec.args, &case.args)),
            };
            match self.runner.run(&request).await {
                Err(error) => {
                    return SuiteOutcome::JudgeFailed {
                        passed,
                        total,
                        detail: describe(&error),
                    };
                }
                Ok(result) => match judge(&result, case.expected.as_deref()) {
                    Verdict::Accepted | Verdict::Finished => passed += 1,
                    Verdict::WrongAnswer | Verdict::Errored => {
                        return SuiteOutcome::Rejected {
                            passed,
                            total,
                            first_failure: FailedCase {
                                index,
                                args: case.args.clone(),
                                expected: case.expected.clone(),
                                stdout: result.stdout,
                                stderr: result.stderr,
                                status: result.status,
                            },
                        };
                    }
                },
            }
        }
        SuiteOutcome::Accepted { total }
    }
}

fn describe(error: &ExecutionError) -> String {
    match error {
        ExecutionError::UnknownLanguage(alias) => format!("unknown language '{alias}'"),
        ExecutionError::PayloadTooLarge { field, .. } => format!("{field} too large"),
        ExecutionError::BackendUnavailable(_) => "execution backend unavailable".to_owned(),
        ExecutionError::BackendFailed(_) => "execution backend failed".to_owned(),
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
