//! Live sandbox ITs, gated behind `GOJUDGE_IT` — need `docker compose up -d go-judge`
//! (host :5150). Run:
//! `GOJUDGE_IT=1 EXECUTOR_URL=http://localhost:5150 cargo test --test go_judge_it -- --test-threads=1`

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use synapse_server::execution::application::{CodeRunner, RunCodeService};
use synapse_server::execution::infrastructure::GoJudgeRunner;
use synapse_shared::execution::{RunRequest, RunStatus};

fn gated() -> Option<GoJudgeRunner> {
    if std::env::var("GOJUDGE_IT").is_err() {
        eprintln!("skipped (set GOJUDGE_IT=1 with a live go-judge to run)");
        return None;
    }
    let url = std::env::var("EXECUTOR_URL").unwrap_or_else(|_| "http://localhost:5150".to_owned());
    Some(GoJudgeRunner::new(&url))
}

#[tokio::test]
async fn python_prints_to_stdout() {
    let Some(runner) = gated() else { return };
    let result = runner
        .run(
            synapse_server::execution::domain::Language::Python,
            "print(21 * 2)",
            None,
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
        .run(&RunRequest {
            language: "java".to_owned(),
            source: java.to_owned(),
            stdin: Some("21\n".to_owned()),
        })
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
        .run(Language::Java, "class Solution { not java }", None)
        .await
        .unwrap();
    assert_eq!(compile.status, RunStatus::CompileError);
    assert!(!compile.compile_output.is_empty());

    let runtime = runner
        .run(Language::Python, "raise RuntimeError('boom')", None)
        .await
        .unwrap();
    assert_eq!(runtime.status, RunStatus::RuntimeError);
    assert!(runtime.stderr.contains("boom"));
}
