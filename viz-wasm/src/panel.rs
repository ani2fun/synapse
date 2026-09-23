//! The docked panel — the `/viz` page's left pane, and nothing but the canvas.
//!
//! Same player as the Visualise modal, different host: no popup, no scrim, and no source pane,
//! because the page's own workbench IS the source pane. Everything that READS rather than draws
//! — the call stack, the program's output, the input the program is waiting for — is `console.rs`,
//! mounted under that workbench, because those things describe the code and belong beside it.
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
//! reports when one is waiting, and the console asks. Answering re-runs from the top with the
//! answer appended and lands the reader one step PAST the prompt, so the value they typed has
//! visibly been read. That is the only honest way to do it over a batch runner, and it is
//! indistinguishable from having continued.

use crate::engine::graph::VizCases;
use crate::engine::playback::State;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use std::cell::RefCell;

use crate::host::WidgetHost;
use crate::player;
use crate::render::memory as memory_render;
use crate::session::{Run, Session, TraceState};
use crate::transport::TransportBar;

/// Which question the canvas is answering.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lens {
    Structure,
    Memory,
}

/// Where the reader is standing, in the source's own terms — the two arrows a debugger draws.
///
/// A trace event fires BEFORE its line runs, so the step on screen names the line about to
/// execute and its predecessor names the one that just did. Getting that backwards would put both
/// arrows one line late, which is exactly the kind of wrong a reader trusts.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// The line that just executed. `None` on the first step, where nothing has run yet.
    pub executed: Option<i32>,
    /// The line about to execute. `None` when there is nothing traced.
    pub next: Option<i32>,
}

/// Whoever wants to know where the reader is standing. One listener, because there is one panel.
type CursorListener = Box<dyn Fn(Cursor)>;

thread_local! {
    /// The page's painter, installed through `entry`. The panel does not know what an editor is;
    /// it only says which lines it is pointing at, and whoever asked decides what that looks like.
    static CURSOR_LISTENER: RefCell<Option<CursorListener>> = const { RefCell::new(None) };
}

/// Register the one listener that follows the reader's step. A second call replaces the first —
/// there is one panel and one page.
pub fn set_cursor_listener(listener: impl Fn(Cursor) + 'static) {
    CURSOR_LISTENER.with_borrow_mut(|slot| *slot = Some(Box::new(listener)));
}

fn announce(cursor: Cursor) {
    CURSOR_LISTENER.with_borrow(|slot| {
        if let Some(listener) = slot {
            listener(cursor);
        }
    });
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
    /// exception is a RESUME, which is the same story continuing and says where to land; it
    /// leaves BOTH lenses alone, because either of them can be the one the reader answered from.
    pub fn show(self, session: Session) {
        self.case_idx.set(0);
        if self.resume_at.get_untracked().is_none() {
            self.step.set(State::initial(1));
            self.mem_step.set(State::initial(1));
        }
        self.current.set(Some(session));
    }

    /// Take the run off both surfaces — the canvas back to its empty state, the console to
    /// nothing, the editor's arrows to none (the cursor reports no lines once nothing is shown).
    pub fn clear(self) {
        self.resume_at.set(None);
        self.current.set(None);
    }

    /// The playback state of whichever lens is on screen — the one the transport, the prompt and
    /// the editor's arrows all answer to.
    #[must_use]
    pub fn active_step(self) -> RwSignal<State> {
        match self.lens.get() {
            Lens::Structure => self.step,
            Lens::Memory => self.mem_step,
        }
    }

    /// Whether the reader is standing on the LAST step of the lens they are looking at.
    ///
    /// This is where a waiting program's prompt belongs, and nowhere else: the run stopped at its
    /// final step because it wanted a value, so asking earlier would ask for something the
    /// program has not reached, and stepping forward is what brings the reader to the question.
    ///
    /// A lens that drew NOTHING has no position at all, and its idle 1-of-1 is a placeholder
    /// rather than a place — treating it as "the last step" put the prompt on a lens the run
    /// never walked, asking the reader to answer a question they were never shown reaching.
    #[must_use]
    pub fn at_last_step(self) -> bool {
        if !self.lens_draws() {
            return false;
        }
        let state = self.active_step().get();
        state.index + 1 >= state.count
    }

    /// Whether the lens on screen has a run it can actually describe. The structure lens needs a
    /// root it can project and often has none; the memory lens needs only steps.
    #[must_use]
    pub fn lens_draws(self) -> bool {
        let Some(run) = self.ready_run() else {
            return false;
        };
        match self.lens.get() {
            Lens::Structure => run.cases.is_ok(),
            Lens::Memory => !run.memory.is_empty(),
        }
    }

    /// The two lines a debugger points at, for the lens on screen. Reactive: it reads the run,
    /// the lens and that lens's step.
    #[must_use]
    pub fn cursor(self) -> Cursor {
        let Some(session) = self.current.get() else {
            return Cursor::default();
        };
        let TraceState::Ready(run) = session.state.get() else {
            return Cursor::default();
        };
        let index = self.active_step().get().index;
        let line_at: Box<dyn Fn(usize) -> Option<i32>> = match self.lens.get() {
            Lens::Memory => {
                // The program has FINISHED here: the line on this step is the last one that ran,
                // and pointing a next-line arrow at it would say it is about to run again.
                if let Some(last) = run.memory.get(index).filter(|step| step.ends_run) {
                    return Cursor {
                        executed: Some(last.line).filter(|l| *l > 0),
                        next: None,
                    };
                }
                Box::new(move |i| run.memory.get(i).map(|s| s.line))
            }
            Lens::Structure => {
                let Ok(cases) = run.cases else {
                    return Cursor::default();
                };
                let case = self.case_idx.get().min(cases.cases.len().saturating_sub(1));
                Box::new(move |i| cases.cases.get(case)?.steps.get(i).map(|s| s.line))
            }
        };
        Cursor {
            executed: index.checked_sub(1).and_then(&line_at).filter(|l| *l > 0),
            next: line_at(index).filter(|l| *l > 0),
        }
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
    // The canvas owns playback, so it is the canvas that says where the reader is standing. The
    // console displays it and the page paints it into the editor; neither of them has to know how
    // a step maps onto a line.
    Effect::new(move |_| announce(store.cursor()));
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
    view! {
        {move || match state.get() {
            TraceState::Tracing => player::tracing_card(),
            TraceState::Failed(message) => player::failed_card(&message),
            TraceState::Ready(run) => ready(&run, structure, store).into_any(),
        }}
    }
}

fn ready(
    run: &Run,
    structure: crate::engine::vocabulary::VizStructure,
    store: VizPanelStore,
) -> impl IntoView + use<> {
    let VizPanelStore { zoom, diff, lens, .. } = store;
    let run = run.clone();

    // A RUN OPENS ON A LENS THAT CAN ANSWER IT. The structure lens needs a root it can project
    // and often has none — and when it has none it draws an objection, reports no position, and
    // therefore takes the editor's arrows, the call stack, the stepped output and the input
    // prompt down with it, because every one of those follows the lens on screen. The reader is
    // then looking at four broken surfaces when the memory lens, one click away, would have
    // answered all four. Only ever a fallback: a lens the reader chose that still works is left
    // exactly where it is.
    if lens.get_untracked() == Lens::Structure && run.cases.is_err() {
        lens.set(Lens::Memory);
    }

    // Land one step PAST the prompt the reader just answered. The program is deterministic up to
    // that input, so every step before it is the same step — and the one after it is the first
    // that could only happen because of what they typed, which is the whole point of typing it.
    // Applied to whichever lens asked, since the two count different things.
    let resume_steps = run.memory.len();
    let resume_case_steps = run
        .cases
        .as_ref()
        .ok()
        .and_then(|cases| cases.cases.first().map(|graph| graph.steps.len()))
        .unwrap_or(0);
    // A FRESH run that stopped to ask opens where it asked. The question is why the reader is
    // here, and opening at step 1 hands them a puzzle — find the one step that matters — before
    // they can answer it. They can still step back through everything that led there.
    let waiting = run.waiting;
    Effect::new(move |_| {
        let at = match store.resume_at.get_untracked() {
            Some(at) => {
                store.resume_at.set(None);
                at
            }
            None if waiting => usize::MAX,
            None => return,
        };
        let (target, count) = match store.lens.get_untracked() {
            Lens::Structure => (store.step, resume_case_steps),
            Lens::Memory => (store.mem_step, resume_steps),
        };
        target.update(|s| {
            s.count = count.max(1);
            s.index = at.min(s.count - 1);
            s.playing = false;
        });
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
    let lens_run = run;

    view! {
        <div class="viz-panel__ready">
            {lens_switch(lens)}
            {move || match lens.get() {
                Lens::Structure => structure_lens(&lens_run, structure, store),
                Lens::Memory => memory_lens(&lens_run, store),
            }}
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
