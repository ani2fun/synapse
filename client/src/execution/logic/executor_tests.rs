//! Oracle: `CodeExecutorSpec` — every transition, especially the staleness guards.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use synapse_shared::execution::RunStatus;

use super::*;

fn result(stdout: &str) -> synapse_shared::execution::RunResult {
    synapse_shared::execution::RunResult {
        status: RunStatus::Accepted,
        stdout: stdout.to_owned(),
        stderr: String::new(),
        compile_output: String::new(),
        time_seconds: None,
        memory_kb: None,
    }
}

#[test]
fn initial_state_is_idle_readonly_and_empty() {
    let state = ExecutorState::initial("print(1)");
    assert_eq!(state.code, "print(1)");
    assert_eq!(state.run_state, RunState::Idle);
    assert_eq!(state.edit_mode, EditMode::ReadOnly);
    assert_eq!(state.result, None);
    assert_eq!(state.error, None);
}

#[test]
fn started_clears_the_previous_outcome_and_mints_a_new_handle() {
    let done = ExecutorState::initial("x").started();
    let handle = done.run_id;
    let done = done.completed(handle, result("42"));
    let restarted = done.started();
    assert_eq!(restarted.run_state, RunState::Running);
    assert_eq!(restarted.result, None);
    assert_eq!(restarted.error, None);
    assert_ne!(restarted.run_id, handle);
}

#[test]
fn completed_applies_only_on_a_matching_handle() {
    let running = ExecutorState::initial("x").started();
    let live = running.run_id;
    let done = running.completed(live, result("42"));
    assert_eq!(done.run_state, RunState::Done);
    assert_eq!(done.result.as_ref().unwrap().stdout, "42");
}

#[test]
fn stale_results_and_failures_are_no_ops() {
    let first = ExecutorState::initial("x").started();
    let stale_handle = first.run_id;
    let second = first.started(); // restart: the first run's handle is now stale
    let after_stale_result = second.completed(stale_handle, result("stale"));
    assert_eq!(after_stale_result, second, "a stale result must change nothing");
    let after_stale_failure = second.failed(stale_handle, "stale error");
    assert_eq!(after_stale_failure, second);
}

#[test]
fn a_result_for_a_cancelled_run_is_ignored() {
    let running = ExecutorState::initial("x").started();
    let in_flight = running.run_id;
    let cancelled = running.cancel();
    assert_eq!(cancelled.run_state, RunState::Idle);
    let late = cancelled.completed(in_flight, result("late"));
    assert_eq!(late, cancelled, "the cancelled run's result must be dropped");
}

#[test]
fn failed_records_the_error_on_a_matching_handle() {
    let running = ExecutorState::initial("x").started();
    let failed = running.failed(running.run_id, "backend down");
    assert_eq!(failed.run_state, RunState::Done);
    assert_eq!(failed.error.as_deref(), Some("backend down"));
}

#[test]
fn set_code_touches_nothing_else() {
    let running = ExecutorState::initial("a").started();
    let typed = running.set_code("b");
    assert_eq!(typed.code, "b");
    assert_eq!(typed.run_state, RunState::Running);
    assert_eq!(typed.run_id, running.run_id);
}

#[test]
fn edit_mode_toggles_and_cancel_edit_reverts_code_but_keeps_the_result() {
    let state = ExecutorState::initial("authored");
    let editing = state.enter_edit();
    assert_eq!(editing.edit_mode, EditMode::Editing);
    let ran = editing.set_code("hacked").started();
    let done = ran.completed(ran.run_id, result("42"));
    let reverted = done.cancel_edit("authored");
    assert_eq!(reverted.code, "authored");
    assert_eq!(reverted.edit_mode, EditMode::ReadOnly);
    assert_eq!(
        reverted.result.as_ref().unwrap().stdout,
        "42",
        "the result survives"
    );
}

#[test]
fn dirtiness_and_changed_lines_compare_against_the_authored_source() {
    let state = ExecutorState::initial("a\nb\nc");
    assert!(!is_dirty(&state, "a\nb\nc"));
    let edited = state.set_code("a\nX\nc\nd");
    assert!(is_dirty(&edited, "a\nb\nc"));
    assert_eq!(
        changed_line_count(&edited, "a\nb\nc"),
        2,
        "one changed + one added"
    );
}
