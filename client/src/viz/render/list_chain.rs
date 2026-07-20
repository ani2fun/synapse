//! The linked-list compartments widget (oracle: `ListRenderers.list`, step 33): each node a
//! boxed value with NEXT (and PREV when doubly) field compartments, joined by coloured SVG
//! arrows, closed by the ∅ terminator.

use crate::viz::engine::graph::{VizGraph, VizNode, VizStep};
use crate::viz::engine::markers;
use leptos::prelude::*;

use super::{dom, themed};

#[must_use]
pub fn list(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let graph = graph.clone();
    view! {
        <div class="viz-dom-widget viz-list">
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                frame(step)
            }}
        </div>
    }
    .into_any()
}

fn frame(step: &VizStep) -> AnyView {
    let info = crate::viz::engine::shapes::chain(step);
    if info.nodes.is_empty() {
        return view! {
            <div class="viz-list__row">{dom::null_glyph()}<span>" empty"</span></div>
        }
        .into_any();
    }
    let doubly = info.is_doubly;
    let items: Vec<AnyView> = info
        .nodes
        .iter()
        .enumerate()
        .flat_map(|(i, n)| {
            let mut parts = Vec::new();
            if i > 0 {
                parts.push(arrows(doubly));
            }
            parts.push(node(n, doubly, step));
            parts
        })
        .chain([arrows(false), dom::null_glyph()])
        .collect();
    view! { <div class="viz-list__row">{items}</div> }.into_any()
}

fn node(n: &VizNode, doubly: bool, step: &VizStep) -> AnyView {
    let cursors = dom::cursors_by_target(step);
    let on_it = cursors.get(n.id.value());
    let mut class = dom::diff_mods("viz-list__node", &n.id, step);
    if on_it.is_some() {
        class.push_str(" viz-list__node--cursor");
    }
    let badge = on_it.map(|cs| dom::cursor_badge(cs));
    let label = n.label.clone();
    view! {
        <div class=class>
            {badge}
            {doubly.then(|| view! {
                <span class="viz-list__field viz-list__field--prev">"prev"</span>
            })}
            <span class="viz-list__val">{label}</span>
            <span class="viz-list__field viz-list__field--next">"next"</span>
        </div>
    }
    .into_any()
}

fn arrows(doubly: bool) -> AnyView {
    let next_color = themed(markers::canon("next").unwrap_or_default());
    let prev_color = themed(markers::canon("previous").unwrap_or_default());
    view! {
        <span class="viz-list__arrows">
            {arrow_svg(&next_color, false)}
            {doubly.then(|| arrow_svg(&prev_color, true))}
        </span>
    }
    .into_any()
}

fn arrow_svg(color: &str, flip: bool) -> AnyView {
    let (x1, x2, head) = if flip {
        (26.0, 4.0, "M10,1 L2,5 L10,9 z")
    } else {
        (2.0, 24.0, "M18,1 L26,5 L18,9 z")
    };
    let color = color.to_owned();
    view! {
        <svg class="viz-list__arrow" viewBox="0 0 28 10" width="28" height="10">
            <line x1=x1 y1="5" x2=x2 y2="5" stroke=color.clone() stroke-width="1.6"></line>
            <path d=head fill=color></path>
        </svg>
    }
    .into_any()
}
