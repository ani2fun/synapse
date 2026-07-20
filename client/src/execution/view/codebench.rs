//! The popup codebench (qna Q1, option A): ONE near-fullscreen modal with ONE Monaco created
//! on first open and reused forever after (value + tokenizer swap, the step-30 seam). Run +
//! editable stdin + the runnable output panel ride along; Esc closes like every other popup;
//! editing gates on sign-in while Run stays open to everyone. Authors write bare fences — no
//! `run` attribute, no markdown changes.
//!
//! The button that opens it moved into the fence group's header bar in step 41 (`fence_group`);
//! this module keeps the store, the modal, and the alias list that decides which fences get one.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::execution::state::BlockStore;
use crate::execution::view::icons::icon_play;
use crate::execution::view::output::Output;
use crate::identity::state::AuthStore;
use crate::islands::editor::{self, EditorCallbacks, MountedEditor};

// ─────────────────────────────────────────────────────────────────────────────
// THE STORE (the VisualiseModal singleton pattern)
// ─────────────────────────────────────────────────────────────────────────────

/// What the pill hands the modal: the fence's text and its language alias.
#[derive(Clone, PartialEq, Eq)]
pub struct CodebenchRequest {
    pub code: String,
    pub language: String,
}

#[derive(Clone, Copy)]
pub struct CodebenchStore {
    pub current: RwSignal<Option<CodebenchRequest>>,
}

impl CodebenchStore {
    pub fn provide() {
        provide_context(Self {
            current: RwSignal::new(None),
        });
    }

    pub fn from_context() -> Self {
        expect_context::<Self>()
    }

    pub fn open(self, request: CodebenchRequest) {
        crate::log::info(&format!("codebench: opening a {} snippet", request.language));
        self.current.set(Some(request));
    }

    pub fn close(self) {
        self.current.set(None);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DISCOVERY — the "Open editor to try" pills
// ─────────────────────────────────────────────────────────────────────────────

/// A fence the engine can actually run. The alias table lives in `execution::logic::language`,
/// shared with the language preference — one table, so the two cannot drift apart (this
/// function's own copy had been a `sqlite` behind the server since step 40).
pub(crate) fn runnable_fence(lang: &str) -> bool {
    crate::execution::logic::canonical_lang(lang).is_some()
}

// ─────────────────────────────────────────────────────────────────────────────
// THE MODAL — one Monaco, reused forever
// ─────────────────────────────────────────────────────────────────────────────

/// Mounted once in the shell (in-tree — context is reachable). The frame stays in the DOM
/// across opens (hidden, not removed) so the single Monaco instance survives; each open
/// swaps value + tokenizer in place.
#[allow(clippy::too_many_lines)]
#[component]
pub fn CodebenchModal() -> impl IntoView {
    let store = CodebenchStore::from_context();
    let auth = AuthStore::from_context();
    let theme = crate::shell::theme::ThemeStore::from_context();
    let block = BlockStore::new("");
    let stdin = RwSignal::new(String::new());
    let mounted: StoredValue<Option<MountedEditor>, LocalStorage> = StoredValue::new_local(None);
    let editor_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    let esc = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" && store.current.get_untracked().is_some() {
            store.close();
        }
    });
    on_cleanup(move || esc.remove());

    let run = move || {
        let Some(request) = store.current.get_untracked() else {
            return;
        };
        block.launch(
            request.language,
            Some(stdin.get_untracked()).filter(|s| !s.is_empty()),
        );
    };

    // Each open resets the bench to the fence: FSM + buffer + stdin; the editor (if already
    // alive) swaps value + tokenizer in place.
    Effect::new(move |_| {
        let Some(request) = store.current.get() else {
            return;
        };
        block
            .state
            .update(|s| *s = crate::execution::logic::ExecutorState::initial(&request.code));
        stdin.set(String::new());
        mounted.with_value(|editor| {
            if let Some(editor) = editor {
                editor.set_value(&request.code);
                editor.set_language(&request.language);
                editor.set_read_only(!auth.authed());
            }
        });
    });
    // First open mounts the ONE editor; it lives for the rest of the session. `run` is a
    // Copy closure (captures only Copy signals), so it rides into the async block directly.
    Effect::new(move |_| {
        let Some(request) = store.current.get() else {
            return;
        };
        let Some(node) = editor_ref.get() else { return };
        if mounted.read_value().is_some() {
            return;
        }
        spawn_local(async move {
            let callbacks = EditorCallbacks {
                on_change: Box::new(move |code: String| {
                    block.state.update(|s| *s = s.set_code(&code));
                }),
                on_run: Box::new(run),
                on_toggle_edit: Box::new(|| {}),
                on_submit: None,
            };
            let dark = theme.is_dark();
            match editor::mount(
                &node,
                &request.code,
                &request.language,
                !auth.authed(),
                dark,
                callbacks,
            )
            .await
            {
                Ok(handle) => mounted.set_value(Some(handle)),
                Err(error) => leptos::logging::error!("codebench monaco failed: {error:?}"),
            }
        });
    });
    // Signing in mid-session unlocks the buffer in place; the theme follows the toggle.
    Effect::new(move |_| {
        let authed = auth.authed();
        mounted.with_value(|editor| {
            if let Some(editor) = editor {
                editor.set_read_only(!authed);
            }
        });
    });
    Effect::new(move |_| {
        let dark = theme.mode.get() == crate::shell::theme::Mode::Dark;
        mounted.with_value(|editor| {
            if let Some(editor) = editor {
                editor.set_theme(dark);
            }
        });
    });

    let running =
        Memo::new(move |_| block.state.read().run_state == crate::execution::logic::RunState::Running);
    let state_signal: Signal<crate::execution::logic::ExecutorState> =
        Signal::derive(move || block.state.get());
    let run_click = run;
    view! {
        <div class="codebench" class:codebench--open=move || store.current.get().is_some()>
            <div class="codebench__scrim" on:click=move |_| store.close()></div>
            <div class="codebench__frame">
                <div class="codebench__bar">
                    <span class="wb__eyebrow"><span class="wb__prompt">"⤢"</span>" CODEBENCH"</span>
                    <span class="wb__lang-pill">
                        {icon_play("wb__lang-play")}
                        <span>{move || {
                            store
                                .current
                                .get()
                                .map(|r| crate::execution::logic::display_lang(&r.language))
                                .unwrap_or_default()
                        }}</span>
                    </span>
                    <span class="codebench__spacer"></span>
                    <button
                        class="runnable__run"
                        prop:disabled=move || running.get()
                        title="Run (⌘⏎)"
                        on:click=move |_| run_click()
                    >
                        {icon_play("runnable__run-ic")}
                        <span>{move || if running.get() { "Running…" } else { "Run" }}</span>
                    </button>
                    <button class="codebench__close" aria-label="Close (Esc)" on:click=move |_| store.close()>
                        "✕"
                    </button>
                </div>
                {move || (!auth.authed()).then(|| signin_bar(auth))}
                <div class="codebench__editor" node_ref=editor_ref></div>
                <div class="codebench__stdin">
                    <label class="viz-stdin__label">"stdin"</label>
                    <textarea
                        class="viz-stdin__input"
                        rows="2"
                        placeholder="Input the program reads, one line per read"
                        prop:value=move || stdin.get()
                        on:input=move |event| stdin.set(event_target_value(&event))
                    ></textarea>
                </div>
                <div class="codebench__out">
                    <Output state=state_signal spec=None tests=None />
                </div>
            </div>
        </div>
    }
}

/// The anonymous edit-gate banner (oracle: the workbench's `wb__edit-bar`) — Run stays open,
/// editing needs sign-in.
fn signin_bar(auth: AuthStore) -> impl IntoView {
    view! {
        <div class="wb__edit-bar codebench__signin">
            <span class="wb__edit-status">
                <span class="wb__edit-dot"></span>
                "Sign in to edit — you can still Run it as written"
            </span>
            <button class="wb__ghost" on:click=move |_| auth.sign_in()>"Sign in"</button>
        </div>
    }
}
