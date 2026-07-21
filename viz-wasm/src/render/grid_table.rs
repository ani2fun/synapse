//! The 2-D grid table (oracle: `GridRenderers.table`, step 33): shared-border cells with
//! row/column index gutters; holes (a ragged inner list) stay dashed and faded.

use crate::engine::graph::{VizGraph, VizStep};
use leptos::prelude::*;

use crate::engine::shapes;
use crate::render::dom;

#[must_use]
pub fn table(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let graph = graph.clone();
    view! {
        <div class="viz-dom-widget viz-grid">
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                frame(step)
            }}
        </div>
    }
    .into_any()
}

fn frame(step: &VizStep) -> AnyView {
    let rows = shapes::grid_cells(step);
    if rows.is_empty() {
        return view! {
            <div class="viz-grid__empty">{dom::null_glyph()}<span>" empty"</span></div>
        }
        .into_any();
    }
    let cursors = dom::cursors_by_target(step);
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let header: Vec<_> = (0..cols)
        .map(|c| view! { <span class="viz-grid__colidx">{c}</span> })
        .collect();
    let body: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(r, row)| {
            let cells: Vec<AnyView> = row
                .iter()
                .map(|cell| {
                    cell.as_ref().map_or_else(
                        || view! { <span class="viz-grid__cell viz-grid__cell--hole">" "</span> }.into_any(),
                        |n| {
                            let on_it = cursors.get(n.id.value());
                            let mut class = dom::diff_mods("viz-grid__cell", &n.id, step);
                            if on_it.is_some() {
                                class.push_str(" viz-grid__cell--cursor");
                            }
                            let badge = on_it.map(|cs| dom::cursor_badge(cs));
                            let label = n.label.clone();
                            view! { <span class=class>{badge}{label}</span> }.into_any()
                        },
                    )
                })
                .collect();
            view! {
                <div class="viz-grid__row">
                    <span class="viz-grid__rowidx">{r}</span>
                    {cells}
                </div>
            }
        })
        .collect();
    view! {
        <div class="viz-grid__row viz-grid__row--header">
            <span class="viz-grid__corner">" "</span>
            {header}
        </div>
        {body}
    }
    .into_any()
}
