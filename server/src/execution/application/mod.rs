//! The execution use case (oracle: `RunCodeService` + the `CodeRunner` port). Deliberately
//! thin: validate → resolve → run. No go-judge knowledge, no concurrency gate — the adapter
//! owns those.

use synapse_shared::execution::{RunRequest, RunResult};

use crate::execution::domain::GO_JUDGE_LIMITS;

use crate::execution::domain::Language;

/// What execution needs from the sandbox. Returns a `RunResult` EVEN WHEN the program failed
/// (crash → `RuntimeError`, compile fail → `CompileError`) — only backend-machinery failures
/// use the error channel.
pub trait CodeRunner: Send + Sync {
    fn run(
        &self,
        language: Language,
        source: &str,
        stdin: Option<&str>,
    ) -> impl Future<Output = Result<RunResult, ExecutionError>> + Send;
}

/// The context's error. HTTP mapping (at `http/`, step 10): `UnknownLanguage`→422,
/// `PayloadTooLarge`→413, `BackendUnavailable`→503, `BackendFailed`→502.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExecutionError {
    #[error("language '{0}' is not runnable")]
    UnknownLanguage(String),
    #[error("{field} too large: {bytes} bytes exceeds the {limit}-byte cap")]
    PayloadTooLarge {
        field: &'static str,
        bytes: usize,
        limit: usize,
    },
    #[error("execution backend unavailable: {0}")]
    BackendUnavailable(String),
    #[error("execution backend failed: {0}")]
    BackendFailed(String),
}

pub struct RunCodeService<R> {
    runner: R,
}

impl<R: CodeRunner> RunCodeService<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Validate → resolve → run. Byte caps are UTF-8 byte counts, INCLUSIVE (`> limit` fails).
    ///
    /// `skip(self, request)` then re-adding the size: the request carries the user's SOURCE
    /// CODE, which is both large and theirs. The byte count is the operationally useful part
    /// and the only part worth keeping.
    #[tracing::instrument(
        name = "execution.run",
        skip(self, request),
        fields(language = %request.language, source_bytes = request.source.len())
    )]
    pub async fn run(&self, request: &RunRequest) -> Result<RunResult, ExecutionError> {
        let language = Language::resolve(&request.language)
            .ok_or_else(|| ExecutionError::UnknownLanguage(request.language.clone()))?;
        ensure_within("Source", &request.source, GO_JUDGE_LIMITS.max_source_bytes)?;
        if let Some(stdin) = &request.stdin {
            ensure_within("Standard input", stdin, GO_JUDGE_LIMITS.max_stdin_bytes)?;
        }
        tracing::debug!(language = language.label(), "running code");
        self.runner
            .run(language, &request.source, request.stdin.as_deref())
            .await
    }
}

fn ensure_within(field: &'static str, value: &str, limit: usize) -> Result<(), ExecutionError> {
    let bytes = value.len();
    if bytes > limit {
        return Err(ExecutionError::PayloadTooLarge { field, bytes, limit });
    }
    Ok(())
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
