//! The playback column — everything a host shows AROUND the canvas, shared by the Visualise
//! modal and the docked `/viz` panel.
//!
//! Both hosts drive the same `Playback` stepper over the same `VizCases`; what differs is only
//! what sits BESIDE the canvas (the modal pairs it with a source pane, the panel with the page's
//! own workbench). So the case strip, the zoom/diff controls, the step timeline, the frames
//! panel, the keyboard bindings and the two failure cards live here, and each host composes them
//! into its own layout.
//!
//! The class names are host-neutral (`viz-controls`, not `viz-modal__controls`) for the same
//! reason: one control row rendered in two places must not carry one place's name.

use crate::engine::graph::{VizCases, VizGraph};
use crate::engine::playback::State;
use leptos::prelude::*;

// ─────────────────────────────────────────────────────────────────────────────
// STATUS CARDS — never a blank box
// ─────────────────────────────────────────────────────────────────────────────

/// What a host shows while the sandbox is still running.
pub fn tracing_card() -> AnyView {
    view! {
        <div class="viz-status">
            <span class="viz-spinner"></span>
            "Tracing your code…"
        </div>
    }
    .into_any()
}

/// The crash, surfaced honestly. The RETRY affordance is the host's — the modal carries a stdin
/// box, the page has one of its own — so this is the message and nothing else.
pub fn failed_card(message: &str) -> AnyView {
    let message = message.to_owned();
    view! {
        <div class="viz-failed">
            <p class="viz-failed__title">"Couldn't visualise this run"</p>
            <pre class="viz-failed__msg">{message}</pre>
        </div>
    }
    .into_any()
}

// ─────────────────────────────────────────────────────────────────────────────
// THE CASE STRIP
// ─────────────────────────────────────────────────────────────────────────────

/// One chip per detected case. Renders nothing for a single-case trace — a strip of one is a
/// label pretending to be a choice.
pub fn case_strip(cases: &VizCases, case_idx: RwSignal<usize>) -> AnyView {
    let count = cases.cases.len();
    if count < 2 {
        return ().into_any();
    }
    let chips: Vec<_> = (0..count)
        .map(|i| {
            let label = format!("Case {}", i + 1);
            view! {
                <button
                    class="viz-case"
                    class:viz-case--active=move || case_idx.get() == i
                    on:click=move |_| case_idx.set(i)
                >
                    {label}
                </button>
            }
        })
        .collect();
    view! { <div class="viz-cases">{chips}</div> }.into_any()
}

// ─────────────────────────────────────────────────────────────────────────────
// CONTROLS · DIFF STOPS
// ─────────────────────────────────────────────────────────────────────────────

/// Zoom out / reset / in. Its own piece because not every canvas has a diff mode to sit beside
/// it — the memory lens draws every step, so there is nothing for it to skip.
pub fn zoom_controls(zoom: RwSignal<f64>) -> impl IntoView {
    view! {
        <div class="viz-zoom">
            <button
                class="viz-zoom__btn"
                aria-label="Zoom out"
                on:click=move |_| zoom.update(|z| *z = (*z - 0.25).max(0.5))
            >
                "−"
            </button>
            <button class="viz-zoom__pct" title="Reset zoom (F)" on:click=move |_| zoom.set(1.0)>
                {move || format!("{:.0}%", zoom.get() * 100.0)}
            </button>
            <button
                class="viz-zoom__btn"
                aria-label="Zoom in"
                on:click=move |_| zoom.update(|z| *z = (*z + 0.25).min(4.0))
            >
                "+"
            </button>
        </div>
    }
}

/// The zoom cluster and the diff toggle.
pub fn controls(zoom: RwSignal<f64>, diff_mode: RwSignal<bool>) -> impl IntoView {
    view! {
        <div class="viz-controls">
            {zoom_controls(zoom)}
            <button
                class="viz-diff"
                class:viz-diff--on=move || diff_mode.get()
                title="Diff mode (D) — step only through frames that changed the structure"
                on:click=move |_| diff_mode.update(|d| *d = !*d)
            >
                {move || if diff_mode.get() { "◧ Diff on" } else { "◧ Diff off" }}
            </button>
        </div>
    }
}

/// The indices where the structure CHANGED — what the transport's step buttons hop between while
/// diff mode is on. Empty when it is off, which is what makes the transport ordinary again.
pub fn diff_stops(
    cases: VizCases,
    case_idx: RwSignal<usize>,
    diff_mode: RwSignal<bool>,
) -> Signal<Vec<usize>> {
    Signal::derive(move || {
        if !diff_mode.get() || cases.cases.is_empty() {
            return Vec::new();
        }
        let idx = case_idx.get().min(cases.cases.len() - 1);
        cases.cases[idx]
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.unchanged)
            .map(|(i, _)| i)
            .collect()
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// KEYS
// ─────────────────────────────────────────────────────────────────────────────

/// Space plays, ←/→ step, `f` resets zoom, `d` toggles diff, `r` re-traces. Typing surfaces are
/// ignored — a reader editing code must not have the space bar swallowed by the player.
pub fn wire_keys(
    step_state: RwSignal<State>,
    zoom: RwSignal<f64>,
    diff_mode: RwSignal<bool>,
    on_retrace: impl Fn() + 'static,
) {
    let handle = window_event_listener(leptos::ev::keydown, move |event| {
        use wasm_bindgen::JsCast;
        let typing = event
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|el| {
                matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA") || el.class_name().contains("monaco")
            });
        if typing || event.meta_key() || event.ctrl_key() {
            return;
        }
        match event.key().as_str() {
            " " => {
                event.prevent_default();
                step_state.update(|s| *s = s.toggle_play());
            }
            "ArrowRight" => step_state.update(|s| *s = s.next()),
            "ArrowLeft" => step_state.update(|s| *s = s.previous()),
            "f" | "F" => zoom.set(1.0),
            "d" | "D" => diff_mode.update(|d| *d = !*d),
            "r" | "R" => on_retrace(),
            _ => {}
        }
    });
    on_cleanup(move || handle.remove());
}

// ─────────────────────────────────────────────────────────────────────────────
// TIMELINE · FRAMES · OUTPUT
// ─────────────────────────────────────────────────────────────────────────────

/// The numbered step chips — reach ANY frame, independent of diff mode; structurally
/// unchanged steps grey out.
pub fn timeline(graph: &VizGraph, step_state: RwSignal<State>) -> AnyView {
    let ticks: Vec<_> = graph
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let unchanged = step.unchanged;
            let title = if unchanged {
                format!("Step {} (no structural change)", i + 1)
            } else {
                format!("Step {}", i + 1)
            };
            view! {
                <button
                    class="viz-timeline__tick"
                    class:viz-timeline__tick--unchanged=unchanged
                    class:viz-timeline__tick--active=move || step_state.get().index == i
                    title=title
                    on:click=move |_| step_state.update(|s| {
                        *s = s.jump_to(i64::try_from(i).unwrap_or(0));
                    })
                >
                    {i + 1}
                </button>
            }
        })
        .collect();
    view! { <div class="viz-timeline not-prose">{ticks}</div> }.into_any()
}

/// The call-stack panel: per-frame fn + locals, the active frame carrying the line chips.
#[component]
pub fn FramesPanel(cases: VizCases, case_idx: RwSignal<usize>, step_state: RwSignal<State>) -> impl IntoView {
    view! {
        <div class="viz-frames">
            {move || {
                let idx = case_idx.get();
                let state = step_state.get();
                let step = cases
                    .cases
                    .get(idx)
                    .and_then(|g: &VizGraph| g.steps.get(state.index));
                let Some(step) = step else {
                    return ().into_any();
                };
                let current = step.line;
                let next_line = cases
                    .cases
                    .get(idx)
                    .and_then(|g: &VizGraph| g.steps.get(state.index + 1))
                    .map(|s| s.line);
                step.frames
                    .iter()
                    .map(|frame| {
                        let class = if frame.is_active { "viz-frame viz-frame--active" } else { "viz-frame" };
                        let chips = (frame.is_active && current > 0).then(|| view! {
                            <span class="viz-frame__lines">
                                <span class="viz-frame__line">{format!("L{current}")}</span>
                                {next_line.map(|n| view! {
                                    <span class="viz-frame__line viz-frame__line--next">
                                        {format!("→ L{n}")}
                                    </span>
                                })}
                            </span>
                        });
                        let locals: Vec<_> = frame
                            .locals
                            .iter()
                            .map(|l| {
                                let lclass = if l.changed {
                                    "viz-frame__local viz-frame__local--changed"
                                } else {
                                    "viz-frame__local"
                                };
                                view! {
                                    <div class=lclass>
                                        <span class="viz-frame__local-name">{l.name.clone()}</span>
                                        <span class="viz-frame__local-type">{l.type_name.clone()}</span>
                                        <span class="viz-frame__local-value">{l.value.clone()}</span>
                                    </div>
                                }
                            })
                            .collect();
                        view! {
                            <div class=class>
                                <div class="viz-frame__fn">{frame.fn_name.clone()}{chips}</div>
                                <div class="viz-frame__locals">{locals}</div>
                            </div>
                        }
                    })
                    .collect::<Vec<_>>()
                    .into_any()
            }}
        </div>
    }
}

/// What the program itself printed, collapsed. Empty output says so rather than showing a blank
/// `<pre>` that reads as a rendering fault.
pub fn program_output(program_out: &str) -> impl IntoView + use<> {
    let out = if program_out.trim().is_empty() {
        "(no output)".to_owned()
    } else {
        program_out.to_owned()
    };
    view! {
        <details class="viz-output">
            <summary class="viz-output__summary">"Program output"</summary>
            <pre class="viz-output__pre">{out}</pre>
        </details>
    }
}
