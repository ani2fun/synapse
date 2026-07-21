//! The queue/deque strip and the vertical stack column (oracle: `QueueRenderers`, step 33):
//! shared-border cell strips with end markers (head/tail · front/back · top) coloured
//! through the role tokens, and the flow hint row above.

use crate::engine::graph::{VizGraph, VizNode, VizStep};
use crate::engine::markers;
use leptos::prelude::*;

use super::{dom, themed};

fn role(name: &str) -> String {
    themed(markers::canon(name).unwrap_or_default())
}

/// The FIFO strip; `deque` flips the vocabulary (front/back, both ends open).
#[must_use]
pub fn queue(graph: &VizGraph, step_index: Signal<usize>, deque: bool) -> AnyView {
    let graph = graph.clone();
    view! {
        <div class="viz-dom-widget viz-queue">
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                queue_frame(step, deque)
            }}
        </div>
    }
    .into_any()
}

fn queue_frame(step: &VizStep, deque: bool) -> AnyView {
    let mut cells: Vec<&VizNode> = step.nodes.iter().filter(|n| n.kind == "cell").collect();
    cells.sort_by_key(|n| n.slot.unwrap_or(i32::MAX));
    let (hint_l, hint_r) = if deque {
        ("⇄ front", "back ⇄")
    } else {
        ("dequeue ←", "← enqueue")
    };
    let strip: Vec<AnyView> = if cells.is_empty() {
        vec![dom::null_glyph(), view! { <span>" empty"</span> }.into_any()]
    } else {
        let last = cells.len() - 1;
        cells
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let (marker, color) = match (i == 0, i == last) {
                    (true, true) if deque => (Some("front · back"), Some(role("current"))),
                    (true, true) => (Some("head · tail"), Some(role("head"))),
                    (true, false) if deque => (Some("front"), Some(role("current"))),
                    (true, false) => (Some("head"), Some(role("head"))),
                    (false, true) if deque => (Some("back"), Some(role("current"))),
                    (false, true) => (Some("tail"), Some(role("tail"))),
                    (false, false) => (None, None),
                };
                #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
                cell(n, i as i32, marker, color, step)
            })
            .collect()
    };
    view! {
        <div class="viz-queue__hint"><span>{hint_l}</span><span>{hint_r}</span></div>
        <div class="viz-queue__cells">{strip}</div>
    }
    .into_any()
}

/// The vertical LIFO column: cells top-first (highest slot), TOP marker on the head cell.
#[must_use]
pub fn stack(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let graph = graph.clone();
    view! {
        <div class="viz-dom-widget viz-queue viz-queue--stack">
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                stack_frame(step)
            }}
        </div>
    }
    .into_any()
}

fn stack_frame(step: &VizStep) -> AnyView {
    let mut cells: Vec<&VizNode> = step.nodes.iter().filter(|n| n.kind == "cell").collect();
    // Top first — highest slot leads the column.
    cells.sort_by_key(|n| std::cmp::Reverse(n.slot.unwrap_or(i32::MIN)));
    let strip: Vec<AnyView> = if cells.is_empty() {
        vec![dom::null_glyph(), view! { <span>" empty"</span> }.into_any()]
    } else {
        cells
            .iter()
            .enumerate()
            .map(|(i, n)| {
                #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
                let pos = n.slot.unwrap_or((cells.len() - 1 - i) as i32);
                let marker = (i == 0).then_some("top");
                let color = (i == 0).then(|| role("top"));
                cell(n, pos, marker, color, step)
            })
            .collect()
    };
    view! {
        <div class="viz-queue__hint viz-queue__hint--stack"><span>"push / pop ⇄"</span></div>
        <div class="viz-queue__cells viz-queue__cells--stack">{strip}</div>
    }
    .into_any()
}

fn cell(n: &VizNode, pos: i32, marker: Option<&str>, color: Option<String>, step: &VizStep) -> AnyView {
    let cursors = dom::cursors_by_target(step);
    let on_it = cursors.get(n.id.value());
    let mut class = dom::diff_mods("viz-queue__cell", &n.id, step);
    if marker.is_some() {
        class.push_str(" viz-queue__cell--end");
    }
    let style = color.map(|c| format!("--viz-end-color: {c}"));
    let badge = on_it.map(|cs| dom::cursor_badge(cs));
    let marker_view = marker.map_or_else(
        || view! { <span class="viz-queue__cell-marker viz-queue__cell-marker--blank">" "</span> }.into_any(),
        |m| view! { <span class="viz-queue__cell-marker">{m.to_owned()}</span> }.into_any(),
    );
    let label = n.label.clone();
    view! {
        <div class=class style=style>
            {badge}
            {marker_view}
            <span class="viz-queue__cell-value">{label}</span>
            <span class="viz-queue__cell-index">{pos}</span>
        </div>
    }
    .into_any()
}
