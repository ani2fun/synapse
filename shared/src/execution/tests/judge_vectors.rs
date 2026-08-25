//! Judge/`stdin_for` TWIN test: this crate and the web workbench
//! (`web/src/lib/execution/judge.ts`) must agree on `judge`/`stdin_for` bit-for-bit. Both sides
//! run the identical `shared/test-vectors/judge-vectors.json`, so drift between the Rust source
//! of truth and its TS port is caught mechanically — on either side of the port — rather than
//! trusted to eyeballing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{ArgSpec, RunResult, RunStatus, Verdict, judge, stdin_for};

#[derive(Debug, Deserialize)]
struct Vectors {
    #[serde(rename = "judgeVectors")]
    judge_vectors: Vec<JudgeVector>,
    #[serde(rename = "stdinVectors")]
    stdin_vectors: Vec<StdinVector>,
}

#[derive(Debug, Deserialize)]
struct JudgeVector {
    name: String,
    status: RunStatus,
    stdout: String,
    expected: Option<String>,
    verdict: String,
}

#[derive(Debug, Deserialize)]
struct StdinVector {
    name: String,
    #[serde(rename = "argIds")]
    arg_ids: Vec<String>,
    values: BTreeMap<String, String>,
    expected: String,
}

fn load_vectors() -> Vectors {
    let path = format!("{}/test-vectors/judge-vectors.json", env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&raw).expect("judge-vectors.json must parse")
}

fn make_result(status: RunStatus, stdout: &str) -> RunResult {
    RunResult {
        status,
        stdout: stdout.to_owned(),
        stderr: String::new(),
        compile_output: String::new(),
        time_seconds: None,
        memory_kb: None,
    }
}

fn verdict_name(v: Verdict) -> &'static str {
    match v {
        Verdict::Accepted => "Accepted",
        Verdict::WrongAnswer => "WrongAnswer",
        Verdict::Errored => "Errored",
        Verdict::Finished => "Finished",
    }
}

#[test]
fn judge_matches_every_vector() {
    let vectors = load_vectors();
    assert!(
        !vectors.judge_vectors.is_empty(),
        "the vector file must not be empty"
    );
    for v in vectors.judge_vectors {
        let result = make_result(v.status, &v.stdout);
        let verdict = judge(&result, v.expected.as_deref());
        assert_eq!(verdict_name(verdict), v.verdict, "case: {}", v.name);
    }
}

#[test]
fn stdin_for_matches_every_vector() {
    let vectors = load_vectors();
    assert!(
        !vectors.stdin_vectors.is_empty(),
        "the vector file must not be empty"
    );
    for v in vectors.stdin_vectors {
        let args: Vec<ArgSpec> = v
            .arg_ids
            .iter()
            .map(|id| ArgSpec {
                id: id.clone(),
                label: id.clone(),
                tpe: "text".to_owned(),
                placeholder: None,
            })
            .collect();
        let actual = stdin_for(&args, &v.values);
        assert_eq!(actual, v.expected, "case: {}", v.name);
    }
}
