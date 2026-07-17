//! Reactive per-block execution state (oracle: `WorkbenchCtx`, reduced to step-11 scope). One
//! `BlockStore` per runnable block: the FSM in a signal, the page-local edit unlock, and
//! `launch()` — run the CURRENT buffer, drop stale replies by handle.

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::execution::RunRequest;

use crate::api;
use crate::execution::logic::{ExecutorState, RunState};

#[derive(Clone, Copy)]
pub struct BlockStore {
    pub state: RwSignal<ExecutorState>,
    /// The page-local Edit unlock (⌘E / the Edit button). The identity step adds the auth
    /// gate on top (oracle: `canEditSignal` — authed && unlocked); until then unlock is free.
    pub unlocked: RwSignal<bool>,
}

impl BlockStore {
    pub fn new(source: &str) -> Self {
        Self {
            state: RwSignal::new(ExecutorState::initial(source)),
            unlocked: RwSignal::new(false),
        }
    }

    /// Run the current buffer. Guards like the Run button: a run in flight wins. `stdin` is
    /// the tests panel's seam — the active case's values, shaped by the shared `stdin_for`.
    pub fn launch(self, language: String, stdin: Option<String>) {
        let current = self.state.get_untracked();
        if current.run_state == RunState::Running {
            return;
        }
        let started = current.started();
        let handle = started.run_id;
        let source = started.code.clone();
        crate::log::info(&format!(
            "running {language} block{}",
            if stdin.is_some() { " (with case stdin)" } else { "" }
        ));
        self.state.set(started);
        spawn_local(async move {
            let request = RunRequest {
                language,
                source,
                stdin,
            };
            match api::run(&request).await {
                Ok(result) => {
                    crate::log::debug(&format!("run done: {}", result.status.label()));
                    self.state.update(|s| *s = s.completed(handle, result));
                }
                Err(message) => {
                    crate::log::error(&format!("run failed: {message}"));
                    self.state.update(|s| *s = s.failed(handle, &message));
                }
            }
        });
    }

    /// The ⌘E / Edit-button toggle. Locking back up reverts the buffer to the authored source
    /// (the last result survives — reverting code is not un-running it).
    pub fn toggle_edit(self, authored: &str) {
        if self.unlocked.get_untracked() {
            self.unlocked.set(false);
            self.state.update(|s| *s = s.cancel_edit(authored));
        } else {
            self.unlocked.set(true);
            self.state.update(|s| *s = s.enter_edit());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SUBMIT (oracle: `WorkbenchCtx.submit` + `SubmitState`, step-15 scope)
// ─────────────────────────────────────────────────────────────────────────────

use synapse_shared::submission::{SubmissionDto, SubmitRequestDto};

/// The submit lifecycle the verdict panel renders.
#[derive(Debug, Clone, PartialEq)]
pub enum SubmitState {
    Idle,
    Judging(String),
    Done(Box<SubmissionDto>),
    Failed(String),
}

/// One workbench's submit machinery: POST → poll every 1.2 s (≤ 100 tries), gated by `alive`
/// so an unmounted block stops polling.
#[derive(Clone, Copy)]
pub struct SubmitStore {
    pub state: RwSignal<SubmitState>,
    alive: RwSignal<bool>,
}

impl SubmitStore {
    pub fn new() -> Self {
        let store = Self {
            state: RwSignal::new(SubmitState::Idle),
            alive: RwSignal::new(true),
        };
        on_cleanup(move || store.alive.set(false));
        store
    }

    /// Guarded like the button: one judging at a time.
    pub fn submit(self, path: Vec<String>, language: String, source: String) {
        if matches!(self.state.get_untracked(), SubmitState::Judging(_)) {
            return;
        }
        crate::log::info(&format!("submitting {language} solution for {}", path.join("/")));
        spawn_local(async move {
            let request = SubmitRequestDto {
                path,
                language,
                source,
            };
            let id = match crate::api::submit(&request).await {
                Ok(accepted) => accepted.id,
                Err(message) => {
                    crate::log::error(&format!("submit failed: {message}"));
                    return self.state.set(SubmitState::Failed(message));
                }
            };
            self.state.set(SubmitState::Judging(id.clone()));
            for _ in 0..100 {
                gloo_timers::future::TimeoutFuture::new(1_200).await;
                if !self.alive.get_untracked() {
                    crate::log::debug(&format!("submission {id}: poll cancelled (block unmounted)"));
                    return;
                }
                match crate::api::submission(&id).await {
                    Ok(dto) if dto.status == "completed" => {
                        crate::log::info(&format!(
                            "submission {id}: {} — {}/{}",
                            dto.verdict.as_deref().unwrap_or("?"),
                            dto.passed.unwrap_or(0),
                            dto.total.unwrap_or(0)
                        ));
                        return self.state.set(SubmitState::Done(Box::new(dto)));
                    }
                    Ok(_) => {} // still pending/judging — keep polling
                    Err(message) => {
                        crate::log::error(&format!("submit failed: {message}"));
                        return self.state.set(SubmitState::Failed(message));
                    }
                }
            }
            crate::log::error("judging timed out");
            self.state
                .set(SubmitState::Failed("judging timed out".to_owned()));
        });
    }
}

impl Default for SubmitStore {
    fn default() -> Self {
        Self::new()
    }
}
