//! The `CodeRunner` adapter over go-judge (oracle: `GoJudgeRunner.scala`).
//!
//! Load-bearing details, all oracle-derived: the client is FORCED to HTTP/1.1 (go-judge is
//! plaintext h1; h2c-upgrade headers come back as bare 400s); a semaphore bounds fan-out to 8
//! concurrent runs (the rate limiter caps rate, not concurrency — excess queues); the
//! per-request timeout is 100 s so go-judge's own clock limit fires first (a cold scala-cli
//! compile can outlast 30 s — a clean TLE beats an opaque HTTP timeout); connection-level
//! failures degrade to `BackendUnavailable` (503), everything else to `BackendFailed` (502) —
//! a badly-running PROGRAM never reaches the error channel.

use std::time::{Duration, Instant};

use synapse_shared::execution::RunResult;
use tokio::sync::Semaphore;

use crate::execution::application::{CodeRunner, ExecutionError};
use crate::execution::domain::Language;
use crate::execution::infrastructure::recipe::Recipe;
use crate::execution::infrastructure::wire;

const MAX_CONCURRENT_RUNS: usize = 8;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Public so the router's global timeout can be checked against it: the edge must outlast the
/// longest legitimate outbound call, or a slow-but-valid run is cut off at the door and the
/// user sees a timeout instead of a clean TLE. Locked by a test in `platform::limits`.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(100);

pub struct GoJudgeRunner {
    client: reqwest::Client,
    run_url: String,
    permits: Semaphore,
}

impl GoJudgeRunner {
    pub fn new(executor_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .http1_only()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self {
            client,
            run_url: format!("{}{}", executor_url.trim_end_matches('/'), wire::RUN_PATH),
            permits: Semaphore::new(MAX_CONCURRENT_RUNS),
        }
    }
}

impl CodeRunner for GoJudgeRunner {
    // The ADAPTER hop: this is the outbound call to go-judge, where the latency
    // actually lives. Source stays out; the language and byte count do not.
    #[tracing::instrument(
        name = "adapter.go_judge",
        skip(self, source, stdin),
        fields(language = language.label(), source_bytes = source.len())
    )]
    async fn run(
        &self,
        language: Language,
        source: &str,
        stdin: Option<&str>,
    ) -> Result<RunResult, ExecutionError> {
        let _permit = self
            .permits
            .acquire()
            .await
            .map_err(|_| ExecutionError::BackendFailed("runner shut down".to_owned()))?;

        let body = wire::build_request_body(language, source, stdin);
        let compiled = Recipe::for_language(language).compile.is_some();
        let started = Instant::now();

        let response = self
            .client
            .post(&self.run_url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| to_execution_error(&e))?;
        if !response.status().is_success() {
            let status = response.status();
            tracing::warn!(%status, "go-judge returned a non-2xx");
            return Err(ExecutionError::BackendFailed(format!(
                "go-judge returned {status}"
            )));
        }
        let text = response.text().await.map_err(|e| to_execution_error(&e))?;
        let result = wire::parse_run_result(compiled, &text).map_err(ExecutionError::BackendFailed)?;
        tracing::debug!(
            language = language.label(),
            status = result.status.label(),
            elapsed_ms = started.elapsed().as_millis(),
            "run completed"
        );
        Ok(result)
    }
}

fn to_execution_error(error: &reqwest::Error) -> ExecutionError {
    if error.is_connect() {
        ExecutionError::BackendUnavailable(error.to_string())
    } else {
        ExecutionError::BackendFailed(error.to_string())
    }
}
