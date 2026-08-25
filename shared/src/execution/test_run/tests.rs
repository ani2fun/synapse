//! How a test case becomes stdin, and how its result becomes a verdict. The authored JSON writes
//! `type`, not the Rust field name — a suite is written by hand, so the key an author types is
//! part of the contract.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::execution::RunStatus;

fn spec_args() -> Vec<ArgSpec> {
    ["a", "b"]
        .iter()
        .map(|id| ArgSpec {
            id: (*id).to_owned(),
            label: id.to_uppercase(),
            tpe: "int".to_owned(),
            placeholder: None,
        })
        .collect()
}

fn run(status: RunStatus, stdout: &str) -> RunResult {
    RunResult {
        status,
        stdout: stdout.to_owned(),
        stderr: String::new(),
        compile_output: String::new(),
        time_seconds: None,
        memory_kb: None,
    }
}

#[test]
fn stdin_is_one_line_per_declared_arg_in_order_with_trailing_newline() {
    let values = BTreeMap::from([("b".to_owned(), "2".to_owned()), ("a".to_owned(), "1".to_owned())]);
    assert_eq!(stdin_for(&spec_args(), &values), "1\n2\n");
    // A missing value is an EMPTY line, keeping positions aligned.
    let sparse = BTreeMap::from([("b".to_owned(), "2".to_owned())]);
    assert_eq!(stdin_for(&spec_args(), &sparse), "\n2\n");
}

#[test]
fn judging_rules() {
    assert_eq!(
        judge(&run(RunStatus::Accepted, "42\n"), Some("42")),
        Verdict::Accepted
    );
    assert_eq!(
        judge(&run(RunStatus::Accepted, "41"), Some("42")),
        Verdict::WrongAnswer
    );
    assert_eq!(
        judge(&run(RunStatus::RuntimeError, ""), Some("42")),
        Verdict::Errored
    );
    assert_eq!(
        judge(&run(RunStatus::Accepted, "anything"), None),
        Verdict::Finished
    );
}

#[test]
fn the_authored_json_writes_type_not_tpe() {
    let spec: TestSpec = serde_json::from_str(
        r#"{"args":[{"id":"n","label":"N","type":"int"}],"cases":[{"args":{"n":"3"},"expected":"6"}]}"#,
    )
    .unwrap();
    assert_eq!(spec.args[0].tpe, "int");
    let written = serde_json::to_string(&spec).unwrap();
    assert!(written.contains("\"type\":\"int\""));
    assert!(!written.contains("tpe"));
}

#[test]
fn sample_defaults_to_hidden_and_omits_the_key_when_false() {
    // A `.tests.json` case with no `sample` field decodes as hidden (judge-only)…
    let spec: TestSpec = serde_json::from_str(
        r#"{"args":[{"id":"n","label":"N","type":"int"}],"cases":[{"args":{"n":"3"},"expected":"6"}]}"#,
    )
    .unwrap();
    assert!(!spec.cases[0].sample);
    // …and re-serializes with no `sample` key, so hidden cases stay unmarked on the wire.
    assert!(!serde_json::to_string(&spec).unwrap().contains("sample"));
}

#[test]
fn samples_keeps_only_sampled_cases_and_clears_the_flag() {
    let spec: TestSpec = serde_json::from_str(
        r#"{"args":[{"id":"n","label":"N","type":"int"}],"cases":[
            {"args":{"n":"6"},"expected":"[1, 2, 3, 6]","sample":true},
            {"args":{"n":"999"},"expected":"[1, 3, 9, 27, 37, 111, 333, 999]"}
        ]}"#,
    )
    .unwrap();
    let samples = spec.samples();
    assert_eq!(samples.cases.len(), 1, "only the sampled case survives");
    assert_eq!(samples.cases[0].args["n"], "6");
    assert!(!samples.cases[0].sample, "the served marker is cleared");
    assert_eq!(samples.args, spec.args, "declared args are preserved");
    // The hidden case's expected output never appears in the served projection.
    assert!(!serde_json::to_string(&samples).unwrap().contains("999"));
}
