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

    /// Run the current buffer. Guards like the Run button: a run in flight wins.
    pub fn launch(self, language: String) {
        let current = self.state.get_untracked();
        if current.run_state == RunState::Running {
            return;
        }
        let started = current.started();
        let handle = started.run_id;
        let source = started.code.clone();
        self.state.set(started);
        spawn_local(async move {
            let request = RunRequest {
                language,
                source,
                stdin: None,
            };
            match api::run(&request).await {
                Ok(result) => self.state.update(|s| *s = s.completed(handle, result)),
                Err(message) => self.state.update(|s| *s = s.failed(handle, &message)),
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
