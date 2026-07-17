//! The widget host (oracle: `WidgetHost`) — the ONE `VizCases` consumer: title, the scaled
//! canvas, the transport bar (only when there's more than one step), the reactive caption,
//! and the honest failure cards (never a blank box).

use leptos::prelude::*;
use synapse_shared::viz::graph::VizCases;
use synapse_shared::viz::playback::State;
use synapse_shared::viz::vocabulary::VizStructure;

use crate::viz::registry;
use crate::viz::transport::TransportBar;

// Component props are moved by design (leptos owns them for the view's lifetime).
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn WidgetHost(
    name: String,
    structure: Option<VizStructure>,
    cases: Option<VizCases>,
    /// The modal drives the SAME stepper its keyboard reads (external playback).
    #[prop(optional)]
    external: Option<RwSignal<State>>,
    /// The modal shows the data-driven legend; inline widgets never do.
    #[prop(optional)]
    legend: bool,
    /// The modal's zoom (CSS `zoom` on the scale layer — layout-aware, unlike transform).
    #[prop(optional)]
    zoom: Option<RwSignal<f64>>,
    /// Diff-mode stops for the transport's step buttons (the modal's ◧ Diff).
    #[prop(optional)]
    stops: Option<Signal<Vec<usize>>>,
) -> impl IntoView {
    match (structure, cases) {
        (Some(structure), Some(cases)) => {
            let Some(graph) = cases.cases.iter().find(|g| !g.steps.is_empty()).cloned() else {
                return unavailable(structure.token());
            };
            let state = external.unwrap_or_else(|| {
                RwSignal::new(State::initial(i64::try_from(graph.steps.len()).unwrap_or(1)))
            });
            let step_index = Signal::derive(move || state.get().index);
            let Some(canvas) = registry::render(structure, &cases, step_index) else {
                return unavailable(structure.token());
            };
            let title = (!graph.title.is_empty()).then(|| graph.title.clone());
            let multi_step = graph.steps.len() > 1;
            let caption_graph = graph.clone();
            view! {
                <div class="viz-widget-host not-prose">
                    {title.map(|t| view! { <div class="viz-widget-host__title">{t}</div> })}
                    <div class="viz-widget-host__canvas">
                        <div
                            class="viz-widget-host__scale"
                            style=move || zoom.map_or_else(String::new, |z| format!("zoom: {:.2}", z.get()))
                        >
                            {canvas}
                        </div>
                    </div>
                    {multi_step.then(|| match stops {
                        Some(stops) => view! { <TransportBar state=state stops=stops /> }.into_any(),
                        None => view! { <TransportBar state=state /> }.into_any(),
                    })}
                    <div class="viz-widget-host__caption">
                        {move || {
                            let i = state.get().index.min(caption_graph.steps.len() - 1);
                            caption_graph.steps[i].annotation.body.clone()
                        }}
                    </div>
                    {legend.then(|| legend_view(&graph, structure))}
                </div>
            }
            .into_any()
        }
        (None, _) => unavailable(&name),
        (Some(_), None) => bad_payload(&name),
    }
}

/// The data-driven legend (oracle: `DomKit.legend`): rows appear only when some step uses
/// the cue. Diff swatches wear the diff TOKENS (what the renderers tint); cursor/line items
/// wear the marker palette. A doubly list adds the next/prev arrow lines.
fn legend_view(graph: &synapse_shared::viz::graph::VizGraph, structure: VizStructure) -> AnyView {
    use synapse_shared::viz::markers;

    use crate::viz::render::themed;
    let steps = &graph.steps;
    let has_cursors = steps.iter().any(|s| !s.cursor.is_empty());
    let has_new = steps.iter().any(|s| !s.highlight.is_empty());
    let has_changed = steps.iter().any(|s| !s.changed.is_empty());
    let has_removed = steps.iter().any(|s| !s.removed.is_empty());
    let is_doubly = structure == VizStructure::List
        && steps.iter().any(|s| {
            s.edges
                .iter()
                .any(|e| matches!(e.label.as_str(), "prev" | "previous"))
        });
    let swatch = |color: String, dashed: bool, glyph: &'static str, text: &'static str| {
        let mut style = format!("border-color: {color}; color: {color}");
        if dashed {
            style.push_str("; border-style: dashed");
        }
        view! {
            <div class="viz-legend__item">
                <span class="viz-legend__swatch" style=style>{glyph}</span>
                <span class="viz-legend__text">{text}</span>
            </div>
        }
        .into_any()
    };
    let line = |color: String, text: &'static str| {
        view! {
            <div class="viz-legend__item">
                <span class="viz-legend__line" style=format!("background: {color}")></span>
                <span class="viz-legend__text">{text}</span>
            </div>
        }
        .into_any()
    };
    let head = themed(markers::canon("head").unwrap_or_default());
    let mut items: Vec<AnyView> = Vec::new();
    if has_cursors {
        items.push(swatch(head, false, "", "a variable points here"));
    }
    if has_new {
        items.push(swatch(
            "hsl(var(--status-ok, var(--primary)))".to_owned(),
            false,
            "",
            "new this step",
        ));
    }
    if has_changed {
        items.push(swatch(
            "hsl(var(--primary))".to_owned(),
            false,
            "",
            "value changed",
        ));
    }
    if has_removed {
        items.push(swatch(
            "hsl(var(--muted-foreground))".to_owned(),
            true,
            "",
            "removed",
        ));
    }
    if has_cursors {
        items.push(swatch(
            "currentColor".to_owned(),
            false,
            "▾",
            "pointer — labelled with the variable",
        ));
    }
    if is_doubly {
        items.push(line(themed(markers::canon("next").unwrap_or_default()), "next"));
        items.push(line(
            themed(markers::canon("previous").unwrap_or_default()),
            "prev",
        ));
    }
    if items.is_empty() {
        return ().into_any();
    }
    view! { <div class="viz-legend">{items}</div> }.into_any()
}

fn unavailable(name: &str) -> AnyView {
    let copy = format!("The “{name}” widget isn’t available yet.");
    view! {
        <div class="viz-widget-host viz-widget-host--unavailable not-prose"><span>{copy}</span></div>
    }
    .into_any()
}

fn bad_payload(name: &str) -> AnyView {
    let copy = format!("Couldn’t read the “{name}” widget payload.");
    view! {
        <div class="viz-widget-host viz-widget-host--error not-prose"><span>{copy}</span></div>
    }
    .into_any()
}
