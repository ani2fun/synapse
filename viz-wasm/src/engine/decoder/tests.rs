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

// ── how the run ended ─────────────────────────────────────────────────────────

#[test]
fn a_run_that_died_reports_the_exception_that_killed_it() {
    let json = r#"{"steps":[],"error":{"type":"ZeroDivisionError","message":"division by zero","line":6}}"#;
    let trace = decode(&wrap("", json)).unwrap().trace.unwrap();
    let error = trace.error.unwrap();
    assert_eq!(error.kind, "ZeroDivisionError");
    assert_eq!(error.line, 6);
    assert_eq!(error.to_string(), "ZeroDivisionError: division by zero (line 6)");
}

#[test]
fn a_null_error_is_a_program_that_finished_not_an_unknown_one() {
    // The harness writes the key on every run, so `null` has to read as "it was fine" — absent
    // and null must not part ways here.
    let explicit = decode(&wrap("", r#"{"steps":[],"error":null}"#))
        .unwrap()
        .trace
        .unwrap();
    let absent = decode(&wrap("", BARE)).unwrap().trace.unwrap();
    assert!(explicit.error.is_none());
    assert!(absent.error.is_none());
}

#[test]
fn an_error_with_no_line_names_itself_without_inventing_a_place() {
    let json = r#"{"steps":[],"error":{"type":"RecursionError","message":"too deep"}}"#;
    let error = decode(&wrap("", json)).unwrap().trace.unwrap().error.unwrap();
    assert_eq!(error.line, 0);
    assert_eq!(error.to_string(), "RecursionError: too deep");
}

// ── how much had been printed ─────────────────────────────────────────────────

#[test]
fn every_step_carries_how_far_the_output_had_got() {
    let json = r#"{"steps":[{"line":1,"out":0},{"line":2,"out":7},{"line":3,"out":14}]}"#;
    let trace = decode(&wrap("tick 0\ntick 1\n", json)).unwrap().trace.unwrap();
    assert_eq!(trace.steps.iter().map(|s| s.out).collect::<Vec<_>>(), [0, 7, 14]);
}

#[test]
fn a_step_that_names_no_offset_has_printed_nothing() {
    // The Java harness records none of this; nothing printed is the only honest reading, and it
    // is what an offset of 0 means anyway.
    let trace = decode(&wrap("", r#"{"steps":[{"line":1}]}"#))
        .unwrap()
        .trace
        .unwrap();
    assert_eq!(trace.steps[0].out, 0);
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

// ── functions, classes and comprehensions ─────────────────────────────────────

#[test]
fn a_function_and_a_class_decode_typed_rather_than_as_instances() {
    let json = r#"{"steps":[{"line":1,"event":"line","frames":[],"heap":{
        "1":{"type":"class","name":"Solution","members":{"spiralOrder":{"ref":"2"}}},
        "2":{"type":"function","sig":"spiralOrder(self, matrix)"}}}]}"#;
    let trace = decode(&wrap("", json)).unwrap().trace.unwrap();
    let heap = &trace.steps[0].heap;
    assert_eq!(
        heap["1"],
        HeapObject::Class {
            name: "Solution".to_owned(),
            members: vec![("spiralOrder".to_owned(), HeapValue::Ref("2".to_owned()))],
        }
    );
    assert_eq!(
        heap["2"],
        HeapObject::Function {
            signature: "spiralOrder(self, matrix)".to_owned(),
        }
    );
}

#[test]
fn a_comprehension_arrives_apart_from_its_owners_locals_and_absent_reads_as_none() {
    let json = r#"{"steps":[
        {"line":56,"event":"line","heap":{},"frames":[{"fn":"<module>","locals":{"rows":3},"comp":{"r":1}}]},
        {"line":57,"event":"line","heap":{},"frames":[{"fn":"<module>","locals":{"rows":3}}]}]}"#;
    let trace = decode(&wrap("", json)).unwrap().trace.unwrap();
    let during = &trace.steps[0].frames[0];
    assert_eq!(
        during.locals,
        [("rows".to_owned(), HeapValue::Scalar(HeapScalar::I(3)))]
    );
    assert_eq!(
        during.comprehension,
        [("r".to_owned(), HeapValue::Scalar(HeapScalar::I(1)))]
    );
    assert!(trace.steps[1].frames[0].comprehension.is_empty());
}
