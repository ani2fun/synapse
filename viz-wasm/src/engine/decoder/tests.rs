//! Splitting a run's stdout into what the program printed and what the harness reported.

#![allow(clippy::unwrap_used)]

use super::*;

fn wrap(program_out: &str, json: &str) -> String {
    format!("{program_out}\n{HEAP_BEGIN}{json}{HEAP_END}\n")
}

const BARE: &str = r#"{"steps":[],"truncated":false}"#;

#[test]
fn a_run_with_no_markers_is_all_program_output_and_no_trace() {
    let decoded = decode("hello\n").unwrap();
    assert_eq!(decoded.program_out, "hello");
    assert!(decoded.trace.is_none());
}

#[test]
fn the_program_output_stops_at_the_marker() {
    let decoded = decode(&wrap("5", BARE)).unwrap();
    assert_eq!(decoded.program_out, "5");
}

// ── the input report ──────────────────────────────────────────────────────────

#[test]
fn the_values_input_was_served_come_back_in_order_with_the_step_that_read_each() {
    let json = r#"{"steps":[],"inputs":[{"v":"[1,2]","at":3},{"v":"7","at":41}],"waiting":false}"#;
    let trace = decode(&wrap("", json)).unwrap().trace.unwrap();
    assert_eq!(
        trace.inputs,
        [
            Served {
                value: "[1,2]".to_owned(),
                at: 3
            },
            Served {
                value: "7".to_owned(),
                at: 41
            },
        ]
    );
    assert!(!trace.waiting);
}

#[test]
fn an_input_that_names_no_step_is_placed_at_the_first_one() {
    // Placing it later would hide it from every step the reader can stand on.
    let trace = decode(&wrap("", r#"{"steps":[],"inputs":[{"v":"7"}]}"#))
        .unwrap()
        .trace
        .unwrap();
    assert_eq!(
        trace.inputs,
        [Served {
            value: "7".to_owned(),
            at: 0
        }]
    );
}

#[test]
fn a_program_that_ran_out_of_input_says_so_and_says_what_it_asked() {
    let json = r#"{"steps":[],"inputs":[],"waiting":true,"prompt":"Your name? "}"#;
    let trace = decode(&wrap("", json)).unwrap().trace.unwrap();
    assert!(trace.waiting);
    assert_eq!(trace.prompt, "Your name? ");
}

#[test]
fn a_trace_that_reports_no_input_at_all_is_a_program_that_asked_for_none() {
    // The Java harness emits none of these fields; absent must not read as "waiting".
    let trace = decode(&wrap("", BARE)).unwrap().trace.unwrap();
    assert!(trace.inputs.is_empty());
    assert!(!trace.waiting);
    assert_eq!(trace.prompt, "");
}

// ── the failure modes ─────────────────────────────────────────────────────────

#[test]
fn a_begin_with_no_end_is_output_the_sandbox_cut_off() {
    let cut = format!("out\n{HEAP_BEGIN}{{\"steps\":[]");
    assert_eq!(decode(&cut), Err(TraceError::TruncatedOutput));
}

#[test]
fn the_last_marker_wins_so_a_program_cannot_spoof_one() {
    let printed = format!("{HEAP_BEGIN}{{\"inputs\":[{{\"v\":\"spoofed\"}}]}}{HEAP_END}");
    let real = r#"{"steps":[],"inputs":[{"v":"real","at":0}]}"#;
    let trace = decode(&wrap(&printed, real)).unwrap().trace.unwrap();
    assert_eq!(
        trace.inputs,
        [Served {
            value: "real".to_owned(),
            at: 0
        }]
    );
}
