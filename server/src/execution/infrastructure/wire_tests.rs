//! Golden request shapes and response mappings for the go-judge wire protocol, no HTTP.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::Value;
use synapse_shared::execution::RunStatus;

use super::*;

fn body_json(language: Language, source: &str, stdin: Option<&str>) -> Value {
    serde_json::from_str(&build_request_body(language, source, stdin)).unwrap()
}

// ── request shapes ────────────────────────────────────────────────────────────

#[test]
fn interpreted_requests_carry_the_raw_source_and_no_compile_markers() {
    let body = body_json(Language::Python, "print('hi')", Some("42\n"));
    let cmd = &body["cmd"][0];
    assert_eq!(cmd["args"][0], "/bin/sh");
    assert_eq!(cmd["args"][1], "-c");
    assert_eq!(cmd["args"][2], "python3 main.py");
    assert_eq!(cmd["files"][0]["content"], "42\n");
    assert_eq!(cmd["files"][1]["name"], "stdout");
    assert_eq!(cmd["files"][1]["max"], 1_048_576);
    assert_eq!(cmd["copyIn"]["main.py"]["content"], "print('hi')");
    assert_eq!(cmd["copyOut"], serde_json::json!([]));
    assert_eq!(cmd["procLimit"], 256);
    assert_eq!(cmd["cpuLimit"], 15_000_000_000_u64);
    assert_eq!(cmd["clockLimit"], 30_000_000_000_u64);
    assert_eq!(cmd["memoryLimit"], 512 * 1024 * 1024);
}

#[test]
fn compiled_requests_use_the_marker_file_trick_and_normalise_java() {
    let body = body_json(
        Language::Java,
        "class Solution { void go() { new Solution(); } }",
        None,
    );
    let cmd = &body["cmd"][0];
    let shell = cmd["args"][2].as_str().unwrap();
    assert!(shell.starts_with("javac Main.java 2>__cf_cerr; echo $? >__cf_crc;"));
    assert!(shell.contains("if [ \"$(cat __cf_crc)\" != \"0\" ]; then exit 0; fi; java -cp . Main"));
    let source = cmd["copyIn"]["Main.java"]["content"].as_str().unwrap();
    assert_eq!(source, "class Main { void go() { new Main(); } }");
    assert_eq!(cmd["copyOut"], serde_json::json!(["__cf_crc?", "__cf_cerr?"]));
    assert_eq!(cmd["files"][0]["content"], "", "absent stdin is an empty content");
}

// ── response mappings ─────────────────────────────────────────────────────────

fn response(status: &str, exit: i64, extra_files: &str) -> String {
    format!(
        r#"[{{"status":"{status}","exitStatus":{exit},"time":12000000,"memory":5632000,
            "files":{{"stdout":"42\n","stderr":""{extra_files}}}}}]"#
    )
}

#[test]
fn accepted_with_exit_zero_maps_units() {
    let result = parse_run_result(false, &response("Accepted", 0, "")).unwrap();
    assert_eq!(result.status, RunStatus::Accepted);
    assert_eq!(result.stdout, "42\n");
    assert_eq!(result.time_seconds, Some(0.012));
    assert_eq!(result.memory_kb, Some(5500));
}

#[test]
fn nonzero_exit_is_a_runtime_error_even_when_accepted() {
    assert_eq!(
        parse_run_result(false, &response("Nonzero Exit Status", 1, ""))
            .unwrap()
            .status,
        RunStatus::RuntimeError
    );
    assert_eq!(
        parse_run_result(false, &response("Accepted", 1, ""))
            .unwrap()
            .status,
        RunStatus::RuntimeError
    );
}

#[test]
fn tle_and_internal_map_directly() {
    assert_eq!(
        parse_run_result(false, &response("TimeLimitExceeded", 0, ""))
            .unwrap()
            .status,
        RunStatus::TimeLimitExceeded
    );
    for backend in ["InternalError", "FileError"] {
        assert_eq!(
            parse_run_result(false, &response(backend, 0, "")).unwrap().status,
            RunStatus::InternalError
        );
    }
}

#[test]
fn a_nonzero_compile_rc_is_a_compile_error_with_blanked_streams() {
    let body = response(
        "Accepted",
        0,
        r#","__cf_crc":"2\n","__cf_cerr":"Main.java:1: error""#,
    );
    let result = parse_run_result(true, &body).unwrap();
    assert_eq!(result.status, RunStatus::CompileError);
    assert_eq!(result.compile_output, "Main.java:1: error");
    assert_eq!(result.stdout, "");
    assert_eq!(result.stderr, "");
}

#[test]
fn a_zero_compile_rc_is_a_clean_run() {
    let body = response("Accepted", 0, r#","__cf_crc":"0\n","__cf_cerr":"""#);
    assert_eq!(parse_run_result(true, &body).unwrap().status, RunStatus::Accepted);
}

#[test]
fn omitted_measurements_are_none_not_zero() {
    let body = r#"[{"status":"Accepted","exitStatus":0,"files":{"stdout":"","stderr":""}}]"#;
    let result = parse_run_result(false, body).unwrap();
    assert_eq!(result.time_seconds, None);
    assert_eq!(result.memory_kb, None);
}

#[test]
fn malformed_json_is_an_error() {
    assert!(parse_run_result(false, "not json").is_err());
    assert!(parse_run_result(false, "[]").is_err());
}
