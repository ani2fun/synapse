//! The one pure step-through state machine — because it's pure it's unit-tested
//! without a DOM or a timer. Named `Playback`, deliberately NOT "FSM" — the codebase already
//! has a real finite-state machine (`CodeExecutor`), and overloading the term would confuse
//! the two.

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
mod tests;
