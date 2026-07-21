//! The submission use cases (`SubmitSolution` + its ports). Anonymous-first: `user_id` stays
//! `None` until an authenticated caller supplies one; the ports already carry the seams
//! (`by_user`, owner checks) so identity slots in without reshaping the aggregate.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use synapse_shared::execution::{RunRequest, TestSpec, Verdict, judge, stdin_for};
use uuid::Uuid;

use crate::execution::application::{CodeRunner, ExecutionError, RunCodeService};
use crate::submission::domain::{FailedCase, Submission, SubmissionId, SubmissionState, SuiteOutcome};

/// The context's error. HTTP mapping (at `http/`): `NotAProblem`/`UnknownSubmission`→404,
/// `InvalidSuite`/`StoreFailed`→500, `SubmitRequiresSignIn`→401, `NotAllowlisted`→403.
/// A program failing its cases is NOT an error — it is a completed submission with a
/// `Rejected` outcome.
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
    #[error("submission '{0}' belongs to someone else")]
    NotYours(String),
    #[error("submitting requires signing in")]
    SubmitRequiresSignIn,
    #[error("'{0}' is not on the submit allowlist")]
    NotAllowlisted(String),
}

/// One grant, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowlistEntry {
    pub username: String,
    pub note: Option<String>,
    pub granted_at: chrono::DateTime<chrono::Utc>,
}

/// Who may SAVE attempts (`SubmissionAllowlist`): keyed by the lowercase IdP username. The
/// admin-panel management verbs live on the same port — one capability, four verbs.
pub trait SubmissionAllowlist: Send + Sync {
    fn is_allowed(&self, username: &str) -> impl Future<Output = Result<bool, SubmissionError>> + Send;
    /// Newest grant first (`granted_at desc, username`).
    fn list(&self) -> impl Future<Output = Result<Vec<AllowlistEntry>, SubmissionError>> + Send;
    /// Upsert — re-granting refreshes the note; returns the stored row.
    fn grant(
        &self,
        username: &str,
        note: Option<&str>,
    ) -> impl Future<Output = Result<AllowlistEntry, SubmissionError>> + Send;
    /// `false` when there was nothing to revoke.
    fn revoke(&self, username: &str) -> impl Future<Output = Result<bool, SubmissionError>> + Send;
}

/// The verified caller, projected for submissions: `user_id` = the stored `sub`,
/// `username` = the (lowercase) allowlist key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submitter {
    pub user_id: String,
    pub username: String,
}

/// The submissions store (`SubmissionRepository`). Owner checks are the APPLICATION's
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
    fn delete(&self, id: SubmissionId) -> impl Future<Output = Result<(), SubmissionError>> + Send;
    /// "Reset my data" — returns the row count.
    fn delete_all_for(&self, user_id: &str) -> impl Future<Output = Result<usize, SubmissionError>> + Send;
    /// Rows still unfinished (`Pending`/`Judging`) that were created before `cutoff`.
    ///
    /// A judging task lives inside the process that spawned it, so a pod killed mid-judge takes
    /// the in-task `JudgeFailed` backstop with it and the row stays unfinished forever. These
    /// are the survivors — see `SubmitSolution::reconcile_unfinished`.
    fn unfinished_before(
        &self,
        cutoff: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Submission>, SubmissionError>> + Send;
}

/// Where a problem's hidden suite comes from (`ProblemTests`) — `None` = not a problem.
pub trait ProblemTests: Send + Sync {
    fn suite_for(
        &self,
        lesson_path: &[String],
    ) -> impl Future<Output = Result<Option<TestSpec>, SubmissionError>> + Send;
}

/// Submit → 202 → background judge → poll. Cloning shares the same adapters (`Arc`s), which is
/// what lets the judge run as a DETACHED task outliving the request.
pub struct SubmitSolution<Repo, Tests, R: CodeRunner, List> {
    repo: Arc<Repo>,
    tests: Arc<Tests>,
    runner: Arc<RunCodeService<R>>,
    allowlist: Arc<List>,
    /// Dev/personal instances stay open (default false); prod flips it on
    /// (`SUBMISSION_ALLOWLIST_ENFORCED`) — saving uses shared compute + storage.
    allowlist_enforced: bool,
}

impl<Repo, Tests, R: CodeRunner, List> Clone for SubmitSolution<Repo, Tests, R, List> {
    fn clone(&self) -> Self {
        Self {
            repo: Arc::clone(&self.repo),
            tests: Arc::clone(&self.tests),
            runner: Arc::clone(&self.runner),
            allowlist: Arc::clone(&self.allowlist),
            allowlist_enforced: self.allowlist_enforced,
        }
    }
}

impl<Repo, Tests, R, List> SubmitSolution<Repo, Tests, R, List>
where
    Repo: SubmissionRepository + Send + Sync + 'static,
    Tests: ProblemTests + Send + Sync + 'static,
    R: CodeRunner + Send + Sync + 'static,
    List: SubmissionAllowlist + Send + Sync + 'static,
{
    pub fn new(
        repo: Arc<Repo>,
        tests: Arc<Tests>,
        runner: Arc<RunCodeService<R>>,
        allowlist: Arc<List>,
        allowlist_enforced: bool,
    ) -> Self {
        Self {
            repo,
            tests,
            runner,
            allowlist,
            allowlist_enforced,
        }
    }

    /// The gate runs FIRST (`authorize`): enforced → anonymous cannot save (401) and
    /// only allow-listed usernames may (403) — rejects never touch the suite or the store.
    async fn authorize(&self, submitter: Option<&Submitter>) -> Result<(), SubmissionError> {
        if !self.allowlist_enforced {
            return Ok(());
        }
        let Some(submitter) = submitter else {
            return Err(SubmissionError::SubmitRequiresSignIn);
        };
        if self.allowlist.is_allowed(&submitter.username).await? {
            Ok(())
        } else {
            Err(SubmissionError::NotAllowlisted(submitter.username.clone()))
        }
    }

    /// Store `Pending`, fire the judge as a detached task, answer immediately (the 202).
    // skip(source): the submitted solution is the user's, and large.
    #[tracing::instrument(
        name = "submission.submit",
        skip(self, source, submitter),
        fields(lesson = %lesson_path.join("/"), %language, source_bytes = source.len())
    )]
    pub async fn submit(
        &self,
        lesson_path: Vec<String>,
        language: String,
        source: String,
        submitter: Option<Submitter>,
    ) -> Result<SubmissionId, SubmissionError> {
        self.authorize(submitter.as_ref()).await?;
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
            user_id: submitter.map(|s| s.user_id),
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

    /// Owner-only: anonymous rows belong to nobody and cannot be deleted.
    pub async fn delete(&self, id: SubmissionId, caller_id: &str) -> Result<(), SubmissionError> {
        let submission = self.get(id).await?;
        if submission.user_id.as_deref() != Some(caller_id) {
            return Err(SubmissionError::NotYours(id.to_string()));
        }
        self.repo.delete(id).await
    }

    pub async fn erase_all_for(&self, user_id: &str) -> Result<usize, SubmissionError> {
        self.repo.delete_all_for(user_id).await
    }

    /// Boot-time reconciliation: complete every row left unfinished by a process that died.
    ///
    /// `judge_and_complete`'s backstop covers judging *failures*, but it runs inside the
    /// detached task — kill the process (a rolling update, an eviction, an OOM) and the task
    /// vanishes mid-flight, leaving the row `Judging` with nothing left to finish it. Nothing
    /// else sweeps them, so the client polls a row that will never terminate.
    ///
    /// Called once at startup. `older_than` must exceed the slowest realistic judge run, or a
    /// restart would fail a suite that a *different* replica is still legitimately running.
    /// Returns how many rows were healed.
    pub async fn reconcile_unfinished(&self, older_than: Duration) -> Result<usize, SubmissionError> {
        let stale = self.repo.unfinished_before(Utc::now() - older_than).await?;
        let mut healed = 0;
        for submission in stale {
            // The suite size makes the verdict read "0/11" rather than a bare "0/0"; it is
            // presentation only, so a lookup failure must not stop the sweep.
            let total = self
                .tests
                .suite_for(&submission.lesson_path)
                .await
                .ok()
                .flatten()
                .map_or(0, |spec| spec.cases.len());
            let outcome = SuiteOutcome::JudgeFailed {
                passed: 0,
                total,
                detail: "judging stopped when the server restarted — submit again".to_owned(),
            };
            match self.repo.update(&submission.completed(outcome, Utc::now())).await {
                Ok(()) => healed += 1,
                Err(error) => {
                    tracing::warn!(id = %submission.id, %error, "could not reconcile an unfinished submission");
                }
            }
        }
        if healed > 0 {
            tracing::info!(healed, "reconciled submissions left unfinished by a previous run");
        }
        Ok(healed)
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
