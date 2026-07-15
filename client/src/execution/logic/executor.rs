//! The `CodeExecutor` FSM (oracle: `shared/execution/CodeExecutor.scala`) — pure
//! `State → State` transitions for one runnable block. The staleness trick: there is no real
//! HTTP cancel, so `cancel`/`started` bump the opaque `RunHandle` and a late result whose
//! handle no longer matches is a NO-OP (`completed`/`failed` return the state unchanged).

use synapse_shared::execution::RunResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Idle,
    Running,
    Done,
}

/// Orthogonal to `RunState`; the auth gate is enforced by the CALLER (the identity step), not
/// by the FSM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    ReadOnly,
    Editing,
}

/// Opaque, monotonic — cannot be fabricated outside this module, so a stored handle can only
/// have come from `started`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunHandle(u64);

impl RunHandle {
    const INITIAL: RunHandle = RunHandle(0);

    fn next(self) -> RunHandle {
        RunHandle(self.0 + 1)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutorState {
    pub code: String,
    pub run_state: RunState,
    pub edit_mode: EditMode,
    pub result: Option<RunResult>,
    pub error: Option<String>,
    pub run_id: RunHandle,
}

impl ExecutorState {
    #[must_use]
    pub fn initial(source: &str) -> Self {
        Self {
            code: source.to_owned(),
            run_state: RunState::Idle,
            edit_mode: EditMode::ReadOnly,
            result: None,
            error: None,
            run_id: RunHandle::INITIAL,
        }
    }

    /// Reset-to-starter: identical to `initial` (kept as its own verb, like the oracle).
    #[must_use]
    pub fn reset(source: &str) -> Self {
        Self::initial(source)
    }

    /// A run begins: clear the previous outcome, mint the handle the eventual result must show.
    #[must_use]
    pub fn started(&self) -> Self {
        Self {
            run_state: RunState::Running,
            result: None,
            error: None,
            run_id: self.run_id.next(),
            ..self.clone()
        }
    }

    /// Cancel without a real HTTP cancel: back to Idle and BUMP the handle, so the in-flight
    /// run's eventual result is stale on arrival.
    #[must_use]
    pub fn cancel(&self) -> Self {
        Self {
            run_state: RunState::Idle,
            run_id: self.run_id.next(),
            ..self.clone()
        }
    }

    /// Apply a result — only if it belongs to the CURRENT run.
    #[must_use]
    pub fn completed(&self, handle: RunHandle, result: RunResult) -> Self {
        if self.run_id != handle {
            return self.clone();
        }
        Self {
            run_state: RunState::Done,
            result: Some(result),
            ..self.clone()
        }
    }

    /// Apply a failure — same staleness guard.
    #[must_use]
    pub fn failed(&self, handle: RunHandle, error: &str) -> Self {
        if self.run_id != handle {
            return self.clone();
        }
        Self {
            run_state: RunState::Done,
            error: Some(error.to_owned()),
            ..self.clone()
        }
    }

    /// Buffer edits touch NOTHING else — a keystroke during a run must not eat the result.
    #[must_use]
    pub fn set_code(&self, code: &str) -> Self {
        Self {
            code: code.to_owned(),
            ..self.clone()
        }
    }

    #[must_use]
    pub fn enter_edit(&self) -> Self {
        Self {
            edit_mode: EditMode::Editing,
            ..self.clone()
        }
    }

    /// Leave edit mode reverting the buffer to the authored source — the last RESULT survives
    /// (reverting code is not un-running it).
    #[must_use]
    pub fn cancel_edit(&self, source: &str) -> Self {
        Self {
            code: source.to_owned(),
            edit_mode: EditMode::ReadOnly,
            ..self.clone()
        }
    }
}

pub fn is_dirty(state: &ExecutorState, source: &str) -> bool {
    state.code != source
}

/// How many lines differ from the authored source, by index (the Edit chrome's badge).
pub fn changed_line_count(state: &ExecutorState, source: &str) -> usize {
    let current: Vec<&str> = state.code.split('\n').collect();
    let authored: Vec<&str> = source.split('\n').collect();
    let max = current.len().max(authored.len());
    (0..max).filter(|&i| current.get(i) != authored.get(i)).count()
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;
