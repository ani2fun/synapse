//! Step transport as a pure state machine: every move pauses, every move clamps to the ends, and
//! the step count floors at one so an empty trace still has somewhere to sit.

use super::*;

fn state(index: usize, playing: bool, count: usize) -> State {
    State {
        index,
        playing,
        count,
    }
}

#[test]
fn initial_is_step_one_paused_count_floored_at_one() {
    assert_eq!(State::initial(5), state(0, false, 5));
    assert_eq!(State::initial(0), state(0, false, 1));
    assert_eq!(State::initial(-3), state(0, false, 1));
}

#[test]
fn next_advances_one_and_pauses_clamping_at_the_last_step() {
    assert_eq!(state(0, true, 3).next(), state(1, false, 3));
    assert_eq!(state(2, true, 3).next(), state(2, false, 3));
}

#[test]
fn previous_retreats_one_and_pauses_clamping_at_the_first_step() {
    assert_eq!(state(2, true, 3).previous(), state(1, false, 3));
    assert_eq!(state(0, true, 3).previous(), state(0, false, 3));
}

#[test]
fn reset_returns_to_the_first_step_paused() {
    assert_eq!(state(2, true, 3).reset(), state(0, false, 3));
}

#[test]
fn jump_to_clamps_into_range_and_pauses() {
    assert_eq!(state(0, true, 4).jump_to(2), state(2, false, 4));
    assert_eq!(state(0, true, 4).jump_to(99), state(3, false, 4));
    assert_eq!(state(2, true, 4).jump_to(-5), state(0, false, 4));
}

#[test]
fn toggle_play_flips_playing_when_not_at_the_end() {
    assert_eq!(state(1, false, 3).toggle_play(), state(1, true, 3));
    assert_eq!(state(1, true, 3).toggle_play(), state(1, false, 3));
}

#[test]
fn pressing_play_at_the_end_rewinds_first() {
    assert_eq!(state(2, false, 3).toggle_play(), state(0, true, 3));
}

#[test]
fn tick_advances_while_playing() {
    assert_eq!(state(0, true, 3).tick(), state(1, true, 3));
}

#[test]
fn tick_stops_at_the_end() {
    assert_eq!(state(2, true, 3).tick(), state(2, false, 3));
}

#[test]
fn tick_is_a_no_op_when_paused() {
    assert_eq!(state(1, false, 3).tick(), state(1, false, 3));
}

#[test]
fn at_start_at_end_read_the_boundaries_including_single_step() {
    assert!(state(0, false, 3).at_start());
    assert!(!state(1, false, 3).at_start());
    assert!(state(2, false, 3).at_end());
    assert!(state(0, false, 1).at_start() && state(0, false, 1).at_end());
}
