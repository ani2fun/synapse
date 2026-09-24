//! The Visualise modal: a near-fullscreen player over the SAME `WidgetHost` the inline
//! widgets use — the case strip, the EDITABLE source pane with current/next line
//! highlights, the frames panel, and
//! the program output. Esc closes; Space toggles play; ←/→ step. The pane's edits and the
//! stdin box feed one LIVE (source, stdin) pair — every re-trace path (the bar's ↻, the `r`
//! key, the stdin panel's button) re-traces exactly what is on screen.
//!
//! Everything it shows AROUND the canvas comes from `player`, which the docked `/viz` panel
//! composes differently. What is the modal's alone is the frame, the source pane beside the
//! canvas, and the stdin that pairs with it.

use crate::engine::graph::VizCases;
use crate::engine::playback::State;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::ffi::editor::{self, EditorCallbacks, MountedEditor};
use crate::host::WidgetHost;
use crate::player::{self, FramesPanel};
use crate::session::{self, Session, TraceState};

/// What the modal shows (set by the workbench's Visualise button).
#[derive(Clone)]
pub struct ModalSession {
    pub session: Session,
}

/// The modal's LIVE inputs: the source pane's buffer + the stdin box. Seeded from the
/// session's key on open; every re-trace path reads THESE, so edits in the popup are what
/// gets traced.
#[derive(Clone, Copy)]
struct LiveInput {
    source: RwSignal<String>,
    stdin: RwSignal<String>,
}

impl LiveInput {
    fn seeded(key: &session::Key) -> Self {
        Self {
            source: RwSignal::new(key.source.clone()),
            stdin: RwSignal::new(key.stdin.clone()),
        }
    }

    /// The session key with the live buffer + stdin swapped in.
    fn fresh_key(self, key: &session::Key) -> session::Key {
        let mut fresh = key.clone();
        fresh.source = self.source.get_untracked();
        fresh.stdin = self.stdin.get_untracked();
        fresh
    }
}

#[derive(Clone, Copy)]
pub struct VizModalStore {
    pub current: RwSignal<Option<ModalSession>>,
}

impl VizModalStore {
    /// A fresh store, owned by the caller's reactive scope. `entry` mints it under a detached
    /// root owner so its signal outlives every view; [`provide`](Self::provide) is the in-tree
    /// alternative for a host that mounts the modal inside a live App.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: RwSignal::new(None),
        }
    }

    pub fn provide() {
        provide_context(Self::new());
    }
}

impl Default for VizModalStore {
    fn default() -> Self {
        Self::new()
    }
}

impl VizModalStore {
    pub fn from_context() -> Self {
        expect_context::<Self>()
    }

    pub fn open(self, session: Session) {
        self.current.set(Some(ModalSession { session }));
    }

    pub fn close(self) {
        self.current.set(None);
    }
}

/// Mounted once in the shell (like the search palette).
#[component]
pub fn VisualiseModal() -> impl IntoView {
    let store = VizModalStore::from_context();
    let esc = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" && store.current.get_untracked().is_some() {
            store.close();
        }
    });
    on_cleanup(move || esc.remove());
    view! {
        {move || {
            store.current.get().map(|modal| {
                let live = LiveInput::seeded(&modal.session.key);
                view! {
                    <div class="viz-modal">
                        <div class="viz-modal__scrim" on:click=move |_| store.close()></div>
                        <div class="viz-modal__frame">
                            <ModalBar modal=modal.clone() store=store live=live />
                            <div class="viz-modal__body">
                                <ModalBody modal=modal.clone() store=store live=live />
                            </div>
                        </div>
                    </div>
                }
            })
        }}
    }
}

#[component]
fn ModalBar(modal: ModalSession, store: VizModalStore, live: LiveInput) -> impl IntoView {
    let retrace_key = modal.session.key.clone();
    let title = modal.session.key.structure.token();
    let guide_open = RwSignal::new(false);
    view! {
        <div class="viz-modal__bar">
            <span class="viz-modal__eyebrow"><span class="viz-modal__prompt">"◆"</span>" VISUALISE"</span>
            <span class="viz-modal__title">{title}</span>
            <span class="viz-modal__bar-spacer"></span>
            {guide_button(guide_open)}
            <button
                class="viz-modal__retrace"
                title="Run the trace again — with your edits and the stdin below"
                on:click=move |_| {
                    store.open(session::obtain_fresh(live.fresh_key(&retrace_key)));
                }
            >
                "↻ Re-trace"
            </button>
            <button class="viz-modal__close" aria-label="Close" on:click=move |_| store.close()>
                "✕"
            </button>
        </div>
    }
}

#[component]
fn ModalBody(modal: ModalSession, store: VizModalStore, live: LiveInput) -> impl IntoView {
    let state = modal.session.state;
    view! {
        {move || match state.get() {
            TraceState::Tracing => player::tracing_card(),
            TraceState::Failed(message) => view! {
                {player::failed_card(&message)}
                {retry_bar(&modal, store, live)}
            }
            .into_any(),
            // The modal is the STRUCTURE lens and only that, so a trace that adapted to
            // nothing is still a failure here — the memory lens that can always draw it lives
            // on `/viz`, which has room for a second view.
            TraceState::Ready(run) => match &run.cases {
                Ok(cases) => ready(&modal, cases, &run, store, live).into_any(),
                Err(message) => view! {
                    {player::failed_card(message)}
                    {retry_bar(&modal, store, live)}
                }
                .into_any(),
            },
        }}
    }
}

/// A failed trace must never dead-end: the stdin box + re-trace ride along on the Failed
/// card too, so a bad input (or a mid-edit syntax error) is fixable in place.
fn retry_bar(modal: &ModalSession, modal_store: VizModalStore, live: LiveInput) -> impl IntoView + use<> {
    let key = StoredValue::new(modal.session.key.clone());
    view! {
        <div class="viz-stdin viz-stdin--retry">
            <label class="viz-stdin__label">"stdin"</label>
            <textarea
                class="viz-stdin__input"
                rows="2"
                prop:value=move || live.stdin.get()
                on:input=move |event| live.stdin.set(event_target_value(&event))
            ></textarea>
            <button
                class="viz-stdin__retrace"
                title="Trace again with the input above (your code edits are kept)"
                on:click=move |_| {
                    modal_store.open(session::obtain_fresh(live.fresh_key(&key.read_value())));
                }
            >
                "↻ Re-trace"
            </button>
        </div>
    }
}

fn ready(
    modal: &ModalSession,
    cases: &VizCases,
    run: &crate::session::Run,
    modal_store: VizModalStore,
    live: LiveInput,
) -> impl IntoView + use<> {
    let program_out = run.program_out.clone();
    // A program that crashed still traced every step up to the crash, so the modal shows the run
    // — but without this it shows a story that simply stops, which reads as the visualiser giving
    // up rather than the code failing.
    let ended_badly = run.error.as_ref().map(|error| {
        view! { <div class="viz-error viz-error--flat">{player::error_summary(error)}</div> }
    });
    let case_idx = RwSignal::new(0usize);
    let zoom = RwSignal::new(1.0_f64);
    let diff_mode = RwSignal::new(false);
    let cases = cases.clone();
    let key = modal.session.key.clone();
    // One playback per case; switching cases resets it (a fresh graph = a fresh animation).
    let step_state = RwSignal::new(State::initial(
        i64::try_from(cases.cases.first().map_or(1, |g| g.steps.len())).unwrap_or(1),
    ));
    Effect::new(move |prev: Option<usize>| {
        let idx = case_idx.get();
        if prev.is_some_and(|p| p != idx) {
            // count is corrected below when the graph renders
            step_state.set(State::initial(1));
        }
        idx
    });

    let keys_key = modal.session.key.clone();
    player::wire_keys(step_state, zoom, diff_mode, move || {
        modal_store.open(session::obtain_fresh(live.fresh_key(&keys_key)));
    });

    let host_cases = cases.clone();
    let pane_cases = cases.clone();
    let timeline_cases = cases.clone();
    let stops = player::diff_stops(cases.clone(), case_idx, diff_mode);
    view! {
        <div class="viz-modal__ready">
            {player::case_strip(&cases, case_idx)}
            <div class="viz-modal__layout">
                <div class="viz-modal__canvas-col">
                    {player::controls(zoom, diff_mode)}
                    {move || {
                        let idx = case_idx.get().min(host_cases.cases.len() - 1);
                        let graph = host_cases.cases[idx].clone();
                        step_state.update(|s| *s = s.resized(graph.steps.len()));
                        let one = VizCases { cases: vec![graph] };
                        view! {
                            <WidgetHost
                                name="trace".to_owned()
                                structure=Some(key.structure)
                                cases=Some(one)
                                external=step_state
                                legend=true
                                zoom=zoom
                                stops=stops
                            />
                        }
                    }}
                    {move || {
                        let idx = case_idx.get().min(timeline_cases.cases.len() - 1);
                        player::timeline(&timeline_cases.cases[idx], step_state)
                    }}
                </div>
                <div class="viz-modal__side-col">
                    <SourcePane
                        source=key.source.clone()
                        language=key.language.clone()
                        cases=pane_cases
                        case_idx=case_idx
                        step_state=step_state
                        live=live
                    />
                    <FramesPanel cases=cases.clone() case_idx=case_idx step_state=step_state />
                </div>
            </div>
            {ended_badly}
            {output_panel(&program_out, modal.session.key.clone(), modal_store, live)}
        </div>
    }
}

/// EDITABLE Monaco over the RAW pre-wrap source (line numbers align with the captions),
/// with current/next line highlights following the stepper. Edits land in the live buffer —
/// re-trace (↻ / `r` / the stdin panel) runs exactly what's on screen; until then the
/// highlights keep narrating the LAST traced run.
#[component]
fn SourcePane(
    source: String,
    language: String,
    cases: VizCases,
    case_idx: RwSignal<usize>,
    step_state: RwSignal<State>,
    live: LiveInput,
) -> impl IntoView {
    let node_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let mounted: StoredValue<Option<MountedEditor>, LocalStorage> = StoredValue::new_local(None);
    let source_for_mount = source.clone();
    Effect::new(move |_| {
        let Some(node) = node_ref.get() else { return };
        if mounted.read_value().is_some() {
            return;
        }
        let value = source_for_mount.clone();
        let lang = language.clone();
        spawn_local(async move {
            let callbacks = EditorCallbacks {
                on_change: Box::new(move |code: String| live.source.set(code)),
                on_run: Box::new(|| {}),
                on_toggle_edit: Box::new(|| {}),
                on_submit: None,
            };
            let dark = crate::theme::html_is_dark();
            match editor::mount(&node, &value, &lang, false, dark, callbacks).await {
                Ok(handle) => mounted.set_value(Some(handle)),
                Err(error) => leptos::logging::error!("source pane monaco failed: {error:?}"),
            }
        });
    });
    // Highlights re-fire on step AND on late Monaco arrival (mounted is plain storage; the
    // step signal ticks often enough that the first highlight lands right after mount).
    Effect::new(move |_| {
        let idx = case_idx.get();
        let state = step_state.get();
        let Some(graph) = cases.cases.get(idx) else { return };
        let Some(step) = graph.steps.get(state.index) else {
            return;
        };
        // A trace event fires BEFORE its line runs, so the step on screen names the line about to
        // execute and its PREDECESSOR names the one that just did. Reading the following step as
        // "next" would point both arrows one line late.
        let next = u32::try_from(step.line.max(0)).ok().filter(|line| *line > 0);
        let executed = state
            .index
            .checked_sub(1)
            .and_then(|previous| graph.steps.get(previous))
            .and_then(|s| u32::try_from(s.line.max(0)).ok())
            .filter(|line| *line > 0);
        mounted.with_value(|editor| {
            if let Some(editor) = editor {
                editor.set_line_highlights(executed, next);
            }
        });
    });
    on_cleanup(move || mounted.set_value(None));
    view! { <div class="viz-modal__source" node_ref=node_ref></div> }
}

// ─────────────────────────────────────────────────────────────────────────────
// OUTPUT · GUIDE
// ─────────────────────────────────────────────────────────────────────────────

/// Program output (collapsed) + the EDITABLE stdin (the shared live pair): its Re-trace
/// runs a FRESH trace over the pane's current code and the input typed here.
fn output_panel(
    program_out: &str,
    key: crate::session::Key,
    modal_store: VizModalStore,
    live: LiveInput,
) -> impl IntoView + use<> {
    let key = StoredValue::new(key);
    view! {
        <div class="viz-modal__output">
            {player::program_output(program_out)}
            <div class="viz-stdin">
                <label class="viz-stdin__label">"stdin"</label>
                <textarea
                    class="viz-stdin__input"
                    rows="2"
                    prop:value=move || live.stdin.get()
                    on:input=move |event| live.stdin.set(event_target_value(&event))
                ></textarea>
                <button
                    class="viz-stdin__retrace"
                    title="Trace again with the code above and this input"
                    on:click=move |_| {
                        modal_store.open(session::obtain_fresh(live.fresh_key(&key.read_value())));
                    }
                >
                    "↻ Re-trace with this input"
                </button>
            </div>
        </div>
    }
}

/// The (i) guide button + the "How Visualise works" card.
fn guide_button(open: RwSignal<bool>) -> impl IntoView {
    const SECTIONS: [(&str, &str); 4] = [
        (
            "What this is",
            "We ran your code for real and captured the data structure after every line — an \
             actual run, not a simulation.",
        ),
        (
            "Stepping",
            "Move line by line with the transport bar or the ← / → keys; Space plays and \
             pauses. The numbered timeline jumps straight to any step.",
        ),
        (
            "Diff mode",
            "Turn on Diff to hop only between steps where the structure actually changed, \
             skipping line-only steps.",
        ),
        (
            "The legend",
            "The rings and colours under the canvas mark what a variable points at, what's \
             new this step, and which value changed.",
        ),
    ];
    view! {
        <div class="viz-modal__guide-wrap">
            <button
                class="viz-modal__info"
                class:viz-modal__info--on=move || open.get()
                aria-label="How Visualise works"
                title="How Visualise works"
                on:click=move |_| open.update(|o| *o = !*o)
            >
                <svg class="viz-modal__info-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                     stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <circle cx="12" cy="12" r="10"></circle>
                    <path d="M12 16v-4"></path>
                    <path d="M12 8h.01"></path>
                </svg>
            </button>
            {move || open.get().then(|| view! {
                <div class="viz-modal__guide-scrim" on:click=move |_| open.set(false)></div>
                <div class="viz-modal__guide">
                    <h3 class="viz-modal__guide-title">"How Visualise works"</h3>
                    {SECTIONS
                        .iter()
                        .map(|(eyebrow, body)| view! {
                            <div class="viz-modal__guide-section">
                                <div class="viz-modal__guide-eyebrow">{*eyebrow}</div>
                                <div class="viz-modal__guide-body">{*body}</div>
                            </div>
                        })
                        .collect::<Vec<_>>()}
                </div>
            })}
        </div>
    }
}
