//! The docked panel — the `/viz` page's left pane.
//!
//! Same player as the Visualise modal, different host: no popup, no scrim, and no source pane,
//! because the page's own workbench IS the source pane.
//!
//! It shows the run through TWO LENSES, and they answer different questions. STRUCTURE asks what
//! shape the data has — it needs a `viz=` token and a root, and draws one projected structure.
//! MEMORY asks what the program's memory looks like — it needs neither, keeps every frame and
//! every reachable object, and draws an arrow per reference. A program the structure lens cannot
//! describe still has a memory, so the second lens works where the first reports an objection.
//!
//! Each lens keeps its OWN step index. They are not the same steps: the structure lens counts
//! adapted, coalesced, per-case steps and the memory lens counts raw trace steps, so sharing an
//! index would put one of them at step 40 of 16.
//!
//! INPUT is the other thing this host has that the modal does not. The sandbox runs a program
//! once with stdin fixed up front, so nobody can type into a running program — but the harness
//! reports when one is waiting, and this asks. Answering re-runs from the top with the answer
//! appended and lands the reader back on the step they were on, which is indistinguishable from
//! having continued, and is the only honest way to do it over a batch runner.

use crate::engine::graph::VizCases;
use crate::engine::playback::State;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::host::WidgetHost;
use crate::player::{self, FramesPanel};
use crate::render::memory as memory_render;
use crate::session::{self, Run, Session, TraceState};
use crate::transport::TransportBar;

/// Which question the canvas is answering.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lens {
    Structure,
    Memory,
}

/// The panel's whole state. Every signal is minted by `entry` under a DETACHED root owner: the
/// page drives this from JS callbacks, and a signal minted inside one of those dies with the
/// callback's scope (the `session.rs` landmine, in a second place).
#[derive(Clone, Copy)]
pub struct VizPanelStore {
    pub current: RwSignal<Option<Session>>,
    pub case_idx: RwSignal<usize>,
    pub step: RwSignal<State>,
    /// The memory lens's own position — see the module doc.
    pub mem_step: RwSignal<State>,
    pub zoom: RwSignal<f64>,
    pub diff: RwSignal<bool>,
    pub lens: RwSignal<Lens>,
    /// The step to land on once the next run is ready — how answering a prompt reads as
    /// continuing rather than as starting again.
    pub resume_at: RwSignal<Option<usize>>,
}

impl VizPanelStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: RwSignal::new(None),
            case_idx: RwSignal::new(0),
            step: RwSignal::new(State::initial(1)),
            mem_step: RwSignal::new(State::initial(1)),
            zoom: RwSignal::new(1.0),
            diff: RwSignal::new(false),
            lens: RwSignal::new(Lens::Structure),
            resume_at: RwSignal::new(None),
        }
    }

    /// Show a trace. Playback resets — a new run is a new animation, and keeping the old step
    /// index would open the panel part-way through a story the reader has not been told. The
    /// exception is a RESUME, which is the same story continuing and says where to land.
    pub fn show(self, session: Session) {
        self.case_idx.set(0);
        self.step.set(State::initial(1));
        if self.resume_at.get_untracked().is_none() {
            self.mem_step.set(State::initial(1));
        }
        self.current.set(Some(session));
    }

    /// The finished run on screen, if there is one.
    #[must_use]
    pub fn ready_run(self) -> Option<Run> {
        match self.current.get_untracked()?.state.get_untracked() {
            TraceState::Ready(run) => Some(run),
            _ => None,
        }
    }

    /// The structure lens's cases — `None` when the trace is unfinished OR adapted to nothing,
    /// which is what the d2 export asks about.
    #[must_use]
    pub fn ready_cases(self) -> Option<VizCases> {
        self.ready_run()?.cases.ok()
    }
}

impl Default for VizPanelStore {
    fn default() -> Self {
        Self::new()
    }
}

#[component]
pub fn VizPanel() -> impl IntoView {
    let store = expect_context::<VizPanelStore>();
    view! {
        <div class="viz-panel">
            {move || match store.current.get() {
                None => empty_state(),
                Some(session) => view! { <PanelBody session=session store=store /> }.into_any(),
            }}
        </div>
    }
}

/// Before the first trace. It names the two things the reader has to do, in the order the page
/// lays them out — an empty canvas with no instructions reads as a broken canvas.
fn empty_state() -> AnyView {
    view! {
        <div class="viz-panel__empty">
            <p class="viz-panel__empty-title">"Nothing traced yet"</p>
            <p class="viz-panel__empty-body">
                "Write code on the right, choose the structure to watch, then press Trace. \
                 We run it for real and capture the structure after every line."
            </p>
        </div>
    }
    .into_any()
}

#[component]
fn PanelBody(session: Session, store: VizPanelStore) -> impl IntoView {
    let state = session.state;
    let structure = session.key.structure;
    let key = session.key.clone();
    view! {
        {move || match state.get() {
            TraceState::Tracing => player::tracing_card(),
            TraceState::Failed(message) => player::failed_card(&message),
            TraceState::Ready(run) => ready(&run, structure, key.clone(), store).into_any(),
        }}
    }
}

fn ready(
    run: &Run,
    structure: crate::engine::vocabulary::VizStructure,
    key: session::Key,
    store: VizPanelStore,
) -> impl IntoView + use<> {
    let VizPanelStore { zoom, diff, lens, .. } = store;
    let run = run.clone();

    // Land where the reader was before they answered the prompt. Runs once per Ready run: the
    // program is deterministic up to the input it stopped at, so the steps before it are the
    // same steps.
    let resume_steps = run.memory.len();
    Effect::new(move |_| {
        if let Some(at) = store.resume_at.get_untracked() {
            store.resume_at.set(None);
            store.mem_step.update(|s| {
                s.count = resume_steps.max(1);
                s.index = at.min(s.count - 1);
                s.playing = false;
            });
        }
    });

    // `r` re-traces in the modal; here the page owns Trace, so the key is left to the page and
    // this host binds only playback.
    player::wire_keys(store.step, zoom, diff, || {});

    // Switching cases restarts playback. Hoisted OUT of the structure lens so toggling lenses
    // does not stack a fresh effect each time it is rebuilt.
    let case_switch = store.case_idx;
    let structure_step = store.step;
    Effect::new(move |prev: Option<usize>| {
        let idx = case_switch.get();
        if prev.is_some_and(|p| p != idx) {
            structure_step.set(State::initial(1));
        }
        idx
    });

    // `AnyView` is not Clone, so each lens is BUILT on demand rather than built twice and
    // stored — which is also what keeps the hidden lens off the DOM entirely.
    let lens_run = run.clone();
    let program_out = run.program_out.clone();
    let frames_cases = run.cases.clone().ok();
    let case_idx = store.case_idx;
    let step = store.step;

    view! {
        <div class="viz-panel__ready">
            {lens_switch(lens)}
            {move || match lens.get() {
                Lens::Structure => structure_lens(&lens_run, structure, store),
                Lens::Memory => memory_lens(&lens_run, store),
            }}
            {input_strip(&run, key, store)}
            {frames_cases.map(|cases| view! {
                <details class="viz-panel__strip">
                    <summary class="viz-panel__strip-summary">"Call stack"</summary>
                    <FramesPanel cases=cases case_idx=case_idx step_state=step />
                </details>
            })}
            <div class="viz-panel__strip-out">{player::program_output(&program_out)}</div>
        </div>
    }
}

/// The two questions, as two buttons. Named for what they SHOW, not for how they are drawn.
fn lens_switch(lens: RwSignal<Lens>) -> impl IntoView {
    let tab = move |which: Lens, label: &'static str, title: &'static str| {
        view! {
            <button
                class="viz-lens__tab"
                class:viz-lens__tab--on=move || lens.get() == which
                title=title
                on:click=move |_| lens.set(which)
            >
                {label}
            </button>
        }
    };
    view! {
        <div class="viz-lens">
            {tab(Lens::Structure, "Structure", "The data structure you picked, animating")}
            {tab(Lens::Memory, "Frames & objects", "Every frame and every object, with the references between them")}
        </div>
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE LENSES
// ─────────────────────────────────────────────────────────────────────────────

fn structure_lens(
    run: &Run,
    structure: crate::engine::vocabulary::VizStructure,
    store: VizPanelStore,
) -> AnyView {
    let VizPanelStore {
        case_idx,
        step,
        zoom,
        diff,
        ..
    } = store;
    let cases = match &run.cases {
        // Not a dead end any more: the objection names the structure that could not be found,
        // and the other lens is one click away and always works.
        Err(message) => {
            let message = message.clone();
            return view! {
                <div class="viz-panel__objection">
                    {player::failed_card(&message)}
                    <p class="viz-panel__objection-hint">
                        "Frames & objects draws this run whatever shape it is — it needs no structure."
                    </p>
                </div>
            }
            .into_any();
        }
        Ok(cases) => cases.clone(),
    };
    let host_cases = cases.clone();
    let timeline_cases = cases.clone();
    let stops = player::diff_stops(cases.clone(), case_idx, diff);
    view! {
        <>
            {player::case_strip(&cases, case_idx)}
            {player::controls(zoom, diff)}
            <Stage>
                {move || {
                    let idx = case_idx.get().min(host_cases.cases.len() - 1);
                    let graph = host_cases.cases[idx].clone();
                    step.update(|s| {
                        s.count = graph.steps.len().max(1);
                        s.index = s.index.min(s.count - 1);
                    });
                    let one = VizCases { cases: vec![graph] };
                    view! {
                        <WidgetHost
                            name="trace".to_owned()
                            structure=Some(structure)
                            cases=Some(one)
                            external=step
                            legend=true
                            zoom=zoom
                            stops=stops
                        />
                    }
                }}
            </Stage>
            {move || {
                let idx = case_idx.get().min(timeline_cases.cases.len() - 1);
                player::timeline(&timeline_cases.cases[idx], step)
            }}
        </>
    }
    .into_any()
}

fn memory_lens(run: &Run, store: VizPanelStore) -> AnyView {
    let VizPanelStore { mem_step, zoom, .. } = store;
    if run.memory.is_empty() {
        return view! {
            <div class="viz-panel__empty">
                <p class="viz-panel__empty-body">"This run captured no steps to draw."</p>
            </div>
        }
        .into_any();
    }
    let count = run.memory.len();
    mem_step.update(|s| {
        s.count = count;
        s.index = s.index.min(count - 1);
    });
    let steps = run.memory.clone();
    let lines: Vec<i32> = steps.iter().map(|s| s.line).collect();
    let index = Signal::derive(move || mem_step.get().index);
    view! {
        <>
            <div class="viz-controls">
                {player::zoom_controls(zoom)}
                <span class="viz-mem__at">
                    {move || {
                        let i = index.get().min(lines.len().saturating_sub(1));
                        lines.get(i).map_or_else(String::new, |line| format!("line {line}"))
                    }}
                </span>
            </div>
            <Stage>
                <div class="viz-widget-host not-prose">
                    <div class="viz-widget-host__canvas">
                        <div
                            class="viz-widget-host__scale"
                            style=move || format!("zoom: {:.2}", zoom.get())
                        >
                            {memory_render::canvas(steps, index)}
                        </div>
                    </div>
                    <TransportBar state=mem_step />
                </div>
            </Stage>
        </>
    }
    .into_any()
}

// ─────────────────────────────────────────────────────────────────────────────
// INPUT
// ─────────────────────────────────────────────────────────────────────────────

/// What the program has read, and what it is waiting for.
///
/// Shown whenever a run served ANY input, not only when one is pending: the list is the record of
/// how this run came to be, and it is what a re-run replays. Hiding it once the program stops
/// asking would erase the answer to "why is it doing that".
fn input_strip(run: &Run, key: session::Key, store: VizPanelStore) -> AnyView {
    if run.inputs.is_empty() && !run.waiting {
        return ().into_any();
    }
    let served: Vec<_> = run
        .inputs
        .iter()
        .enumerate()
        .map(|(i, value)| {
            view! {
                <li class="viz-input__served">
                    <span class="viz-input__n">{format!("{}.", i + 1)}</span>
                    <code>{value.clone()}</code>
                </li>
            }
        })
        .collect();
    let ask = run.waiting.then(|| {
        let pending = RwSignal::new(String::new());
        let prompt = run.prompt.clone();
        let history = run.inputs.clone();
        let submit = move || {
            let mut next = key.clone();
            next.stdin = session::replay_stdin(&history, &pending.get_untracked());
            // Land back where the reader is standing — the steps before the prompt are the same
            // steps, so this reads as the program carrying on.
            store.resume_at.set(Some(store.mem_step.get_untracked().index));
            store.show(session::obtain(next));
        };
        let on_submit = submit.clone();
        view! {
            <div class="viz-input__ask">
                <label class="viz-input__label">
                    {if prompt.trim().is_empty() {
                        "The program is waiting for input".to_owned()
                    } else {
                        prompt.clone()
                    }}
                </label>
                <div class="viz-input__row">
                    <input
                        class="viz-input__box"
                        autofocus
                        placeholder="Type a line, then Enter"
                        prop:value=move || pending.get()
                        on:input=move |event| pending.set(event_target_value(&event))
                        on:keydown=move |event: web_sys::KeyboardEvent| {
                            if event.key() == "Enter" {
                                event.prevent_default();
                                on_submit();
                            }
                        }
                    />
                    <button class="viz-input__go" on:click=move |_| submit()>
                        "Enter"
                    </button>
                </div>
            </div>
        }
    });
    view! {
        <div class="viz-input">
            {(!served.is_empty()).then(|| view! {
                <div class="viz-input__log">
                    <span class="viz-input__log-title">"Inputs read so far"</span>
                    <ol class="viz-input__list">{served}</ol>
                </div>
            })}
            {ask}
        </div>
    }
    .into_any()
}

// ─────────────────────────────────────────────────────────────────────────────
// THE STAGE
// ─────────────────────────────────────────────────────────────────────────────

/// The pan/zoom surface around the canvas.
///
/// The handlers sit on this wrapper but drive the canvas's OWN scroller
/// (`.viz-widget-host__canvas`), found by query at drag start — so the transport bar and caption,
/// which the host renders below that box, stay put while the drawing moves.
///
/// Plain wheel scrolls. Zoom is ctrl/⌘+wheel, the convention every map and canvas uses; hijacking
/// a bare wheel would trap the page's scroll inside a figure.
#[component]
fn Stage(children: Children) -> impl IntoView {
    let stage: NodeRef<leptos::html::Div> = NodeRef::new();
    let zoom_of = move || expect_context::<VizPanelStore>().zoom;
    // (pointer x, pointer y, scrollLeft, scrollTop) at grab.
    let grip: StoredValue<Option<(f64, f64, f64, f64)>> = StoredValue::new(None);

    let scroller = move || {
        stage
            .get_untracked()
            .and_then(|node| node.query_selector(".viz-widget-host__canvas").ok().flatten())
            .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
    };

    view! {
        <div
            class="viz-panel__stage"
            node_ref=stage
            on:pointerdown=move |event: web_sys::PointerEvent| {
                let Some(box_el) = scroller() else { return };
                event.prevent_default();
                grip.set_value(Some((
                    f64::from(event.client_x()),
                    f64::from(event.client_y()),
                    box_el.scroll_left().into(),
                    box_el.scroll_top().into(),
                )));
            }
            on:pointermove=move |event: web_sys::PointerEvent| {
                let Some((x, y, left, top)) = grip.get_value() else { return };
                let Some(box_el) = scroller() else { return };
                #[allow(clippy::cast_possible_truncation)]
                {
                    box_el.set_scroll_left((left - (f64::from(event.client_x()) - x)) as i32);
                    box_el.set_scroll_top((top - (f64::from(event.client_y()) - y)) as i32);
                }
            }
            on:pointerup=move |_| grip.set_value(None)
            on:pointerleave=move |_| grip.set_value(None)
            on:wheel=move |event: web_sys::WheelEvent| {
                if !(event.ctrl_key() || event.meta_key()) {
                    return;
                }
                event.prevent_default();
                let step = if event.delta_y() < 0.0 { 0.15 } else { -0.15 };
                zoom_of().update(|z| *z = (*z + step).clamp(0.5, 4.0));
            }
        >
            {children()}
        </div>
    }
}
