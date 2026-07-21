//! The one pure step-through state machine (oracle: `Playback.scala`, ADR-S026). Cortex had
//! three stepper implementations; Synapse has one, and because it's pure it's unit-tested
//! without a DOM or a timer. Named `Playback`, deliberately NOT "FSM" — the codebase already
//! has a real finite-state machine (`CodeExecutor`), and overloading the term confuses the
//! two (qna Q36).

/// `index` — the current step, always in `[0, count)`; `playing` — whether a transport timer
/// is advancing; `count` — the number of steps (≥ 1). Construct via [`State::initial`]; the
/// transitions keep `index` in range so illegal states can't arise from stepping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub index: usize,
    pub playing: bool,
    pub count: usize,
}

impl State {
    /// The opening state for `count` steps: first step, paused. `count` floors at 1.
    #[must_use]
    pub fn initial(count: i64) -> Self {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // floored at 1
        Self {
            index: 0,
            playing: false,
            count: count.max(1) as usize,
        }
    }

    /// At the first step — `previous`/first are no-ops here.
    #[must_use]
    pub fn at_start(self) -> bool {
        self.index == 0
    }

    /// At the last step — `next`/last are no-ops, and the play timer stops here.
    #[must_use]
    pub fn at_end(self) -> bool {
        self.index + 1 >= self.count
    }

    fn clamp(self, i: i64) -> usize {
        let top = i64::try_from(self.count.saturating_sub(1)).unwrap_or(i64::MAX);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // clamped to [0, top]
        {
            i.clamp(0, top.max(0)) as usize
        }
    }

    /// Manual forward step — advances one and PAUSES (a manual step always stops autoplay).
    #[must_use]
    pub fn next(self) -> Self {
        Self {
            index: self.clamp(i64::try_from(self.index).unwrap_or(i64::MAX - 1) + 1),
            playing: false,
            ..self
        }
    }

    /// Manual back step — retreats one and pauses.
    #[must_use]
    pub fn previous(self) -> Self {
        Self {
            index: self.clamp(i64::try_from(self.index).unwrap_or(i64::MAX) - 1),
            playing: false,
            ..self
        }
    }

    /// Back to the first step, paused.
    #[must_use]
    pub fn reset(self) -> Self {
        Self {
            index: 0,
            playing: false,
            ..self
        }
    }

    /// Jump to an arbitrary step (clamped), paused — the scrubber and case switches use this.
    #[must_use]
    pub fn jump_to(self, i: i64) -> Self {
        Self {
            index: self.clamp(i),
            playing: false,
            ..self
        }
    }

    /// Play/pause toggle. Pressing play while already at the end REWINDS to the start first,
    /// so a finished animation replays instead of doing nothing.
    #[must_use]
    pub fn toggle_play(self) -> Self {
        if self.playing {
            Self {
                playing: false,
                ..self
            }
        } else if self.at_end() {
            Self {
                index: 0,
                playing: true,
                ..self
            }
        } else {
            Self {
                playing: true,
                ..self
            }
        }
    }

    /// One timer tick: advance while playing, and STOP at the end (the timer clears itself).
    /// A tick while paused is a no-op, so an always-on timer is harmless.
    #[must_use]
    pub fn tick(self) -> Self {
        if !self.playing {
            self
        } else if self.at_end() {
            Self {
                playing: false,
                ..self
            }
        } else {
            Self {
                index: self.index + 1,
                ..self
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
}
