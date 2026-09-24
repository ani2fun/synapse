//! Live sandbox ITs, gated behind `GOJUDGE_IT` — need `docker compose up -d go-judge`
//! (host :5150). Run:
//! `GOJUDGE_IT=1 EXECUTOR_URL=http://localhost:5150 cargo test --test go_judge_it -- --test-threads=1`

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use synapse_server::execution::application::{CodeRunner, RunCodeService};
use synapse_server::execution::domain::Tier;
use synapse_server::execution::infrastructure::GoJudgeRunner;
use synapse_shared::execution::{RunRequest, RunStatus};

fn gated() -> Option<GoJudgeRunner> {
    gated_with_anonymous_share(100)
}

fn gated_with_anonymous_share(percent: u64) -> Option<GoJudgeRunner> {
    if std::env::var("GOJUDGE_IT").is_err() {
        eprintln!("skipped (set GOJUDGE_IT=1 with a live go-judge to run)");
        return None;
    }
    let url = std::env::var("EXECUTOR_URL").unwrap_or_else(|_| "http://localhost:5150".to_owned());
    Some(GoJudgeRunner::new(&url, percent))
}

#[tokio::test]
async fn python_prints_to_stdout() {
    let Some(runner) = gated() else { return };
    let result = runner
        .run(
            synapse_server::execution::domain::Language::Python,
            "print(21 * 2)",
            None,
            Tier::SignedIn,
        )
        .await
        .unwrap();
    assert_eq!(result.status, RunStatus::Accepted);
    assert_eq!(result.stdout, "42\n");
}

#[tokio::test]
async fn the_whole_pipeline_normalises_java_and_reads_stdin() {
    let Some(runner) = gated() else { return };
    let service = RunCodeService::new(runner);
    let java = "import java.util.Scanner;\nclass Solution {\n  public static void main(String[] a) {\n    System.out.println(new Scanner(System.in).nextInt() * 2);\n  }\n}";
    let result = service
        .run(
            &RunRequest {
                language: "java".to_owned(),
                source: java.to_owned(),
                stdin: Some("21\n".to_owned()),
            },
            Tier::SignedIn,
        )
        .await
        .unwrap();
    assert_eq!(result.status, RunStatus::Accepted, "stderr: {}", result.stderr);
    assert_eq!(result.stdout, "42\n");
}

#[tokio::test]
async fn compile_and_runtime_errors_come_back_as_results() {
    use synapse_server::execution::domain::Language;
    let Some(runner) = gated() else { return };
    let compile = runner
        .run(
            Language::Java,
            "class Solution { not java }",
            None,
            Tier::SignedIn,
        )
        .await
        .unwrap();
    assert_eq!(compile.status, RunStatus::CompileError);
    assert!(!compile.compile_output.is_empty());

    let runtime = runner
        .run(
            Language::Python,
            "raise RuntimeError('boom')",
            None,
            Tier::SignedIn,
        )
        .await
        .unwrap();
    assert_eq!(runtime.status, RunStatus::RuntimeError);
    assert!(runtime.stderr.contains("boom"));
}

#[tokio::test]
async fn an_anonymous_run_gets_only_its_share_of_the_clock() {
    use synapse_server::execution::domain::Language;
    // 10% of Python's 30 s is a 3 s clock: a 4 s sleep outlasts it anonymously and fits the
    // signed-in 30 s. Sleep, not a busy loop, so it is the WALL clock being proved.
    let Some(runner) = gated_with_anonymous_share(10) else {
        return;
    };
    let sleepy = "import time\ntime.sleep(4)\nprint('awake')";
    let anonymous = runner
        .run(Language::Python, sleepy, None, Tier::Anonymous)
        .await
        .unwrap();
    assert_eq!(anonymous.status, RunStatus::TimeLimitExceeded);
    let signed_in = runner
        .run(Language::Python, sleepy, None, Tier::SignedIn)
        .await
        .unwrap();
    assert_eq!(signed_in.status, RunStatus::Accepted);
    assert_eq!(signed_in.stdout.trim(), "awake");
}
