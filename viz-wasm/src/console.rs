//! The console — the `/viz` page's RIGHT pane, under the editor.
//!
//! Everything the panel does not draw. The canvas answers "what shape is the data"; this answers
//! the questions a reader asks with one finger on the code: which line am I on, what did the
//! program print, what is on the stack, and what is it waiting for me to type. Those belong
//! beside the source, not beside the picture — which is the whole reason this is a second mount
//! rather than more of `panel.rs`.
//!
//! It shares the panel's store, so the two surfaces cannot disagree: one step index, one lens,
//! one run. The page mounts them into two of its own nodes and never mediates between them.
//!
//! THE PROGRAM'S OUTPUT ARRIVES AS THE READER STEPS, because showing all of it at step 0 hands
//! them the answer before the program has worked it out. Every trace step records how many bytes
//! had been printed by the time it ran, and the strip shows that prefix.
//!
//! A RUN THAT ENDED BADLY says so, in a card that does not wait to be stepped to. An uncaught
//! exception is a fact about the whole run, not about one step, and a reader who has to walk 200
//! steps to discover the story broke has been told nothing useful by the walk.
//!
//! THE PROMPT ONLY APPEARS AT THE LAST STEP, and that is the feature. A waiting run stopped
//! exactly where it wanted a value, so asking any earlier would ask for something the program has
//! not reached — the reader steps forward, arrives at the question, and answers it. Which is what
//! "type it in as the code runs" means when the sandbox underneath is a batch runner that has
//! already finished.

use leptos::prelude::*;

use crate::engine::memory::MemoryStep;
use crate::panel::{Lens, VizPanelStore};
use crate::player::FramesPanel;
use crate::session::{self, Run, Session, TraceState};

#[component]
pub fn VizConsole() -> impl IntoView {
    let store = expect_context::<VizPanelStore>();
    view! {
        <div class="viz-console">
            {move || match store.current.get() {
                None => ().into_any(),
                Some(session) => view! { <ConsoleBody session=session store=store /> }.into_any(),
            }}
        </div>
    }
}

#[component]
fn ConsoleBody(session: Session, store: VizPanelStore) -> impl IntoView {
    let state = session.state;
    let key = session.key.clone();
    view! {
        {move || match state.get() {
            // Tracing and failure both belong on the canvas, which has the room to say why. The
            // console simply has nothing to report about a run that produced nothing.
            TraceState::Tracing | TraceState::Failed(_) => ().into_any(),
            TraceState::Ready(run) => ready(&run, key.clone(), store).into_any(),
        }}
    }
}

fn ready(run: &Run, key: session::Key, store: VizPanelStore) -> impl IntoView + use<> {
    let frames_cases = run.cases.clone().ok();
    let memory = run.memory.clone();
    let case_idx = store.case_idx;
    let step = store.step;
    view! {
        <div class="viz-console__body">
            {error_card(run, store)}
            {cursor_legend(store)}
            {input_strip(run, key, store)}
            <details class="viz-console__strip">
                <summary class="viz-console__strip-summary">"Call stack"</summary>
                // Follows the LENS, because it has to describe the step on screen. The two count
                // different things, so a call stack fixed to one of them would narrate a moment
                // the reader is not standing in — the failure mode is silent, and reads as the
                // program having been somewhere it never was.
                {move || match store.lens.get() {
                    Lens::Memory => memory_frames(&memory, store).into_any(),
                    Lens::Structure => frames_cases.clone().map_or_else(
                        || view! {
                            <p class="viz-console__none">
                                "No structure to follow — switch to Frames & objects."
                            </p>
                        }.into_any(),
                        |cases| view! {
                            <FramesPanel cases=cases case_idx=case_idx step_state=step />
                        }.into_any(),
                    ),
                }}
            </details>
            <div class="viz-console__strip">{output_strip(run, store)}</div>
        </div>
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HOW IT ENDED
// ─────────────────────────────────────────────────────────────────────────────

/// The exception that ended the run, when one did.
///
/// Shown at EVERY step, not only the one it happened on: the reader has to know the story breaks
/// before deciding how carefully to read it, and a card that waits to be stepped to is a card
/// they find after it would have helped. The jump is what ties it back to a moment — the trace
/// ends on the line that raised, so that is where it goes.
fn error_card(run: &Run, store: VizPanelStore) -> AnyView {
    let Some(error) = run.error.clone() else {
        return ().into_any();
    };
    let steps = run.memory.len();
    let line = error.line;
    view! {
        <div class="viz-error">
            <span class="viz-error__kind">{error.kind}</span>
            <span class="viz-error__msg">{error.message}</span>
            {(line > 0).then(|| view! { <span class="viz-error__at">{format!("line {line}")}</span> })}
            {(steps > 0).then(|| view! {
                <button
                    class="viz-error__go"
                    title="Stop where it broke"
                    on:click=move |_| {
                        store.lens.set(Lens::Memory);
                        store.mem_step.update(|s| {
                            s.count = steps;
                            s.index = steps - 1;
                            s.playing = false;
                        });
                    }
                >
                    "Show me"
                </button>
            })}
        </div>
    }
    .into_any()
}

// ─────────────────────────────────────────────────────────────────────────────
// WHAT IT PRINTED
// ─────────────────────────────────────────────────────────────────────────────

/// What the program has printed BY THE STEP ON SCREEN — the prefix, not the whole run.
///
/// Only the memory lens can do that honestly. Its steps are the raw trace, one per recorded
/// event, each carrying the byte count; the structure lens counts adapted, coalesced, per-case
/// steps with no trace step behind them, so an offset there would be invented, and inventing one
/// prints output the program had not reached. That lens gets the whole run, which is what it
/// showed before either way.
fn output_strip(run: &Run, store: VizPanelStore) -> impl IntoView + use<> {
    let full = run.program_out.clone();
    let printed_anything = !full.trim().is_empty();
    let offsets: Vec<usize> = run.memory.iter().map(|step| step.out).collect();
    // The box is short and the interesting line is always the LAST one, so it follows the
    // program down. Without this a run that prints more than a few lines shows its opening
    // forever while the line the reader just executed sits below the fold.
    let pre: NodeRef<leptos::html::Pre> = NodeRef::new();
    Effect::new(move |_| {
        let _ = store.mem_step.get();
        let _ = store.lens.get();
        if let Some(node) = pre.get() {
            node.set_scroll_top(node.scroll_height());
        }
    });
    view! {
        <details class="viz-output" open=printed_anything>
            <summary class="viz-output__summary">"Program output"</summary>
            <pre class="viz-output__pre" node_ref=pre>
                {move || {
                    let shown = match store.lens.get() {
                        Lens::Structure => full.as_str(),
                        Lens::Memory => offsets
                            .get(store.mem_step.get().index)
                            // A byte count the harness recorded against this very string always
                            // lands on a character boundary; `get` refuses rather than panics if
                            // it somehow does not.
                            .and_then(|at| full.get(..*at))
                            .unwrap_or(full.as_str()),
                    };
                    if shown.is_empty() {
                        if printed_anything {
                            "(nothing printed yet)".to_owned()
                        } else {
                            "(no output)".to_owned()
                        }
                    } else {
                        shown.to_owned()
                    }
                }}
            </pre>
        </details>
    }
}

/// The memory lens's own call stack, in the shape the structure lens's uses.
///
/// The canvas draws these frames too, but as boxes a reader has to find. A pointer shows the NAME
/// of what it points at rather than an empty cell: on the canvas the arrow says where it goes, and
/// in a list there is no arrow to follow.
fn memory_frames(steps: &[MemoryStep], store: VizPanelStore) -> impl IntoView + use<> {
    let steps = steps.to_owned();
    view! {
        <div class="viz-frames">
            {move || {
                let index = store.mem_step.get().index;
                let Some(step) = steps.get(index) else {
                    return ().into_any();
                };
                let title_of = |id: &str| {
                    step.objects
                        .iter()
                        .find(|object| object.id == id)
                        .map_or_else(|| "→".to_owned(), |object| format!("→ {}", object.title))
                };
                step.frames
                    .iter()
                    .map(|frame| {
                        let class = if frame.is_active {
                            "viz-frame viz-frame--active"
                        } else {
                            "viz-frame"
                        };
                        let slots: Vec<_> = frame
                            .slots
                            .iter()
                            .map(|slot| {
                                let value = slot
                                    .target
                                    .as_deref()
                                    .map_or_else(|| slot.value.clone(), title_of);
                                let class = if slot.changed {
                                    "viz-frame__local viz-frame__local--changed"
                                } else {
                                    "viz-frame__local"
                                };
                                view! {
                                    <div class=class>
                                        <span class="viz-frame__local-name">{slot.name.clone()}</span>
                                        <span class="viz-frame__local-value">{value}</span>
                                    </div>
                                }
                            })
                            .collect();
                        view! {
                            <div class=class>
                                <div class="viz-frame__fn">{frame.title.clone()}</div>
                                <div class="viz-frame__locals">{slots}</div>
                            </div>
                        }
                    })
                    .collect::<Vec<_>>()
                    .into_any()
            }}
        </div>
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE CURSOR
// ─────────────────────────────────────────────────────────────────────────────

/// What the two highlights in the editor mean, and which lines they are on.
///
/// A coloured line with nothing naming it is decoration. This is the key, and it carries the line
/// numbers too, so a reader whose editor has scrolled away still knows where the program is.
fn cursor_legend(store: VizPanelStore) -> impl IntoView {
    view! {
        <div class="viz-cursor">
            {move || {
                let cursor = store.cursor();
                view! {
                    <span
                        class="viz-cursor__key viz-cursor__key--done"
                        class:viz-cursor__key--off=cursor.executed.is_none()
                    >
                        <i class="viz-cursor__arrow"></i>
                        "line that just executed"
                        {cursor.executed.map(|line| view! {
                            <b class="viz-cursor__at">{format!("{line}")}</b>
                        })}
                    </span>
                    <span
                        class="viz-cursor__key viz-cursor__key--next"
                        class:viz-cursor__key--off=cursor.next.is_none()
                    >
                        <i class="viz-cursor__arrow"></i>
                        "next line to execute"
                        {cursor.next.map(|line| view! {
                            <b class="viz-cursor__at">{format!("{line}")}</b>
                        })}
                    </span>
                }
            }}
        </div>
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// INPUT
// ─────────────────────────────────────────────────────────────────────────────

/// What the program has read, and — once the reader reaches the end of the run — what it is
/// waiting for.
///
/// The log shows whenever a run served ANY value, not only while one is pending: it is the record
/// of how this run came to be, and it is what a re-run replays. A value the reader has not yet
/// stepped past shows PENDING rather than struck through, because at that point in the story the
/// program genuinely has not been told it.
fn input_strip(run: &Run, key: session::Key, store: VizPanelStore) -> AnyView {
    if run.inputs.is_empty() && !run.waiting {
        return ().into_any();
    }
    let inputs = run.inputs.clone();
    let served: Vec<_> = inputs
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, input)| {
            let at = input.at;
            let read_by_now = move || store.active_step().get().index >= at;
            view! {
                <li class="viz-input__served" class:viz-input__served--read=read_by_now>
                    <span class="viz-input__n">{format!("{}.", i + 1)}</span>
                    <code>{input.value}</code>
                </li>
            }
        })
        .collect();
    let ask = run.waiting.then(|| ask_box(run, inputs.clone(), key, store));
    view! {
        <div class="viz-input">
            {(!served.is_empty()).then(|| view! {
                <div class="viz-input__log">
                    <span class="viz-input__log-title">"Inputs you have typed"</span>
                    <ol class="viz-input__list">{served}</ol>
                </div>
            })}
            {ask}
        </div>
    }
    .into_any()
}

/// The prompt itself, shown only while the reader is standing where the program stopped.
///
/// Answering re-runs with every earlier value plus this one and lands one step past the prompt,
/// so the value is visibly consumed rather than the reader being returned to the same question.
fn ask_box(
    run: &Run,
    history: Vec<crate::engine::trace::Served>,
    key: session::Key,
    store: VizPanelStore,
) -> impl IntoView + use<> {
    let pending = RwSignal::new(String::new());
    let prompt = run.prompt.clone();
    let submit = move || {
        let mut next = key.clone();
        next.stdin = session::replay_stdin(&history, &pending.get_untracked());
        let landing = store.active_step().get_untracked();
        store.resume_at.set(Some(landing.index + 1));
        store.show(session::obtain(next));
    };
    let on_enter = submit.clone();
    view! {
        {move || {
            if !store.at_last_step() {
                // Not yet there. Naming the step keeps this from reading as a dead control —
                // the reader is one transport press away from the question.
                return view! {
                    <p class="viz-input__ahead">
                        "Keep stepping — the program asks for input at the last step."
                    </p>
                }
                .into_any();
            }
            let on_enter = on_enter.clone();
            let on_click = on_enter.clone();
            view! {
                <div class="viz-input__ask">
                    <label class="viz-input__label" for="viz-ask">
                        {if prompt.trim().is_empty() {
                            "Enter user input".to_owned()
                        } else {
                            prompt.clone()
                        }}
                    </label>
                    <div class="viz-input__row">
                        <input
                            id="viz-ask"
                            class="viz-input__box"
                            autofocus
                            placeholder="Type a value, then press Enter"
                            prop:value=move || pending.get()
                            on:input=move |event| pending.set(event_target_value(&event))
                            on:keydown=move |event: web_sys::KeyboardEvent| {
                                if event.key() == "Enter" {
                                    event.prevent_default();
                                    on_enter();
                                }
                            }
                        />
                        <button class="viz-input__go" on:click=move |_| on_click()>
                            "Submit"
                        </button>
                    </div>
                </div>
            }
            .into_any()
        }}
    }
}
