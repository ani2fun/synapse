//! Two rules, both about what a reader is told. `replay_stdin` decides whether answering a prompt
//! continues the story or stops at the same prompt again; `outcome` decides whether a run that
//! broke says so or leaves an empty canvas.

#![allow(clippy::panic, clippy::unwrap_used)]

use super::*;

/// A value served at a step. Every test here is about the ORDER of the lines, never the steps,
/// so the step is noise the helper keeps out of the assertions.
fn served(value: &str) -> Served {
    Served {
        value: value.to_owned(),
        at: 0,
    }
}

#[test]
fn the_first_answer_is_the_whole_stdin() {
    assert_eq!(replay_stdin(&[], "7"), "7\n");
}

#[test]
fn earlier_answers_come_back_in_order_before_the_new_one() {
    let earlier = vec![served("[1,2,3]"), served("7")];
    assert_eq!(replay_stdin(&earlier, "yes"), "[1,2,3]\n7\nyes\n");
}

#[test]
fn every_line_is_terminated_including_the_last() {
    // An unterminated final line reads as EOF, which would stop the run at the very prompt the
    // answer was meant to satisfy — the bug this rule exists to prevent.
    let stdin = replay_stdin(&[served("a")], "b");
    assert!(stdin.ends_with('\n'), "{stdin:?}");
    assert_eq!(stdin.lines().count(), 2);
}

#[test]
fn an_empty_answer_is_still_a_line() {
    // Pressing Enter on nothing is a legitimate answer — `input()` returns "".
    assert_eq!(replay_stdin(&[served("a")], ""), "a\n\n");
}

// ── outcome: a run that produced nothing must say why ─────────────────────────

fn key() -> Key {
    Key {
        language: "python".to_owned(),
        source: String::new(),
        structure: VizStructure::Array,
        root: None,
        stdin: String::new(),
    }
}

fn traced(payload: &str) -> String {
    format!("{}{payload}{}\n", decoder::HEAP_BEGIN, decoder::HEAP_END)
}

#[test]
fn a_source_that_never_compiled_fails_with_the_harness_reason_not_a_blank_canvas() {
    // Zero steps and an error is the syntax-error shape. Ready-with-nothing was the old answer,
    // and it showed the reader an empty box with no clue what was wrong.
    let stdout = traced(r#"{"steps":[],"error":{"type":"SyntaxError","message":"invalid syntax","line":3}}"#);
    match outcome(&key(), &stdout, "", "") {
        TraceState::Failed(message) => assert_eq!(message, "SyntaxError: invalid syntax (line 3)"),
        _ => panic!("a run with no steps must not read as Ready"),
    }
}

#[test]
fn the_harness_reason_beats_a_scraped_stderr() {
    // stderr is a traceback through the HARNESS's own frames; the harness knows the user's line.
    let stdout = traced(r#"{"steps":[],"error":{"type":"SyntaxError","message":"bad","line":1}}"#);
    match outcome(
        &key(),
        &stdout,
        "Traceback (most recent call last): File \"/w/main.py\"",
        "",
    ) {
        TraceState::Failed(message) => assert!(message.starts_with("SyntaxError"), "{message}"),
        _ => panic!("expected a failure"),
    }
}

#[test]
fn a_run_with_no_steps_and_no_reason_still_surfaces_whatever_the_stream_said() {
    let stdout = traced(r#"{"steps":[]}"#);
    match outcome(&key(), &stdout, "boom", "") {
        TraceState::Failed(message) => assert_eq!(message, "boom"),
        _ => panic!("expected a failure"),
    }
}

#[test]
fn a_run_that_crashed_part_way_is_still_shown_and_still_carries_the_reason() {
    // The steps up to the crash are the whole point: the reader watches it go wrong.
    let stdout = traced(
        r#"{"steps":[{"line":1,"event":"line","frames":[],"heap":{}}],
            "error":{"type":"ZeroDivisionError","message":"division by zero","line":1}}"#,
    );
    match outcome(&key(), &stdout, "", "") {
        TraceState::Ready(run) => {
            assert_eq!(run.memory.len(), 1);
            assert_eq!(run.error.unwrap().kind, "ZeroDivisionError");
        }
        _ => panic!("a partial trace is worth showing"),
    }
}

// ── the cache's bound ──

#[test]
fn the_cache_lets_go_of_the_oldest_run_past_its_cap() {
    let mut recent = Recent::new(2);
    assert!(recent.insert("a", 1).is_empty());
    assert!(recent.insert("b", 2).is_empty());
    assert_eq!(recent.insert("c", 3), vec![1]);
    assert_eq!(recent.get(&"a"), None);
    assert_eq!(recent.get(&"c"), Some(3));
}

#[test]
fn reopening_a_run_keeps_it_from_being_the_next_one_evicted() {
    let mut recent = Recent::new(2);
    recent.insert("a", 1);
    recent.insert("b", 2);
    assert_eq!(recent.get(&"a"), Some(1));
    assert_eq!(recent.insert("c", 3), vec![2], "b was the least recently used");
}

#[test]
fn replacing_a_key_hands_back_the_run_it_replaced() {
    let mut recent = Recent::new(4);
    recent.insert("a", 1);
    assert_eq!(recent.insert("a", 9), vec![1]);
    assert_eq!(recent.get(&"a"), Some(9));
}
