//! `RunStatus` on the wire: the case name is the serialized form, the label is the human one,
//! and only `Accepted` counts as success. `RunResult` serializes camelCase because the reader
//! consumes it as JSON.

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn only_accepted_is_success_and_labels_read() {
    assert!(RunStatus::Accepted.is_success());
    for status in [
        RunStatus::CompileError,
        RunStatus::RuntimeError,
        RunStatus::TimeLimitExceeded,
        RunStatus::InternalError,
    ] {
        assert!(!status.is_success());
        assert!(!status.label().is_empty());
    }
    assert_eq!(RunStatus::CompileError.label(), "Compilation Error");
}

#[test]
fn run_status_crosses_the_wire_as_the_case_name() {
    assert_eq!(
        serde_json::to_string(&RunStatus::TimeLimitExceeded).unwrap(),
        "\"TimeLimitExceeded\""
    );
    let parsed: RunStatus = serde_json::from_str("\"Accepted\"").unwrap();
    assert_eq!(parsed, RunStatus::Accepted);
}

#[test]
fn run_result_uses_camel_case_field_names() {
    let result = RunResult {
        status: RunStatus::Accepted,
        stdout: "42\n".to_owned(),
        stderr: String::new(),
        compile_output: String::new(),
        time_seconds: Some(0.012),
        memory_kb: Some(5500),
    };
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["status"], "Accepted");
    assert_eq!(json["compileOutput"], "");
    assert_eq!(json["timeSeconds"], 0.012);
    assert_eq!(json["memoryKb"], 5500);
}
