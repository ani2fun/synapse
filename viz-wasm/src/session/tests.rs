//! What a re-run replays. The rest of this module is I/O; this is the rule that decides whether
//! answering a prompt continues the story or stops at the same prompt again.

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
