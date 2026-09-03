//! The docked panel — the `/viz` page's left pane.
//!
//! Same player as the Visualise modal, different host: there is no popup, no scrim, and no source
//! pane, because the page's own workbench IS the source pane. What is left is the canvas and the
//! things that belong to it — the case strip, the controls, the transport, the timeline, the
//! frames and the program's output.
//!
//! It owns no input of its own. The page holds the buffer and the stdin box and calls
//! [`crate::entry::viz_panel_trace`]; a failed trace is therefore never a dead end here without a
//! retry bar of its own — the code and the input are already on screen.

use crate::engine::graph::VizCases;
use crate::engine::playback::State;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::host::WidgetHost;
use crate::player::{self, FramesPanel};
use crate::session::{Session, TraceState};

/// The panel's whole state. Every signal is minted by `entry` under a DETACHED root owner: the
/// page drives this from JS callbacks, and a signal minted inside one of those dies with the
/// callback's scope (the `session.rs` landmine, in a second place).
#[derive(Clone, Copy)]
pub struct VizPanelStore {
    pub current: RwSignal<Option<Session>>,
    pub case_idx: RwSignal<usize>,
    pub step: RwSignal<State>,
    pub zoom: RwSignal<f64>,
    pub diff: RwSignal<bool>,
}

impl VizPanelStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: RwSignal::new(None),
            case_idx: RwSignal::new(0),
            step: RwSignal::new(State::initial(1)),
            zoom: RwSignal::new(1.0),
            diff: RwSignal::new(false),
        }
    }

    /// Show a trace. Playback resets — a new run is a new animation, and keeping the old step
    /// index would open the panel part-way through a story the reader has not been told.
    pub fn show(self, session: Session) {
        self.case_idx.set(0);
        self.step.set(State::initial(1));
        self.current.set(Some(session));
    }

    /// The cases on screen, if the trace has finished. `entry`'s d2 export reads this.
    #[must_use]
    pub fn ready_cases(self) -> Option<VizCases> {
        match self.current.get_untracked()?.state.get_untracked() {
            TraceState::Ready(cases, _) => Some(cases),
            _ => None,
        }
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
    view! {
        {move || match state.get() {
            TraceState::Tracing => player::tracing_card(),
            TraceState::Failed(message) => player::failed_card(&message),
            TraceState::Ready(cases, program_out) => {
                ready(&cases, &program_out, structure, store).into_any()
            }
        }}
    }
}

fn ready(
    cases: &VizCases,
    program_out: &str,
    structure: crate::engine::vocabulary::VizStructure,
    store: VizPanelStore,
) -> impl IntoView + use<> {
    let VizPanelStore {
        case_idx,
        step,
        zoom,
        diff,
        ..
    } = store;
    let cases = cases.clone();
    // Switching case restarts playback; the count is corrected when the graph renders below.
    Effect::new(move |prev: Option<usize>| {
        let idx = case_idx.get();
        if prev.is_some_and(|p| p != idx) {
            step.set(State::initial(1));
        }
        idx
    });
    // `r` re-traces in the modal; here the page owns Trace, so the key is left to the page and
    // this host binds only playback.
    player::wire_keys(step, zoom, diff, || {});

    let host_cases = cases.clone();
    let frames_cases = cases.clone();
    let timeline_cases = cases.clone();
    let stops = player::diff_stops(cases.clone(), case_idx, diff);
    let program_out = program_out.to_owned();
    view! {
        <div class="viz-panel__ready">
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
            <details class="viz-panel__strip">
                <summary class="viz-panel__strip-summary">"Call stack"</summary>
                <FramesPanel cases=frames_cases case_idx=case_idx step_state=step />
            </details>
            <div class="viz-panel__strip-out">{player::program_output(&program_out)}</div>
        </div>
    }
}

/// The pan/zoom surface around the canvas.
///
/// The handlers sit on this wrapper but drive the `WidgetHost`'s OWN scroller
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
