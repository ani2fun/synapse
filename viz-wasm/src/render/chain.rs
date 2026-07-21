//! The Chain family (oracle: `ChainRenderers`): linked-list boxes left-to-right in
//! next-order, `next` arrows above centre, `prev` dashed below, and the `∅` terminator.

use crate::engine::geometry::constants::{CELL_H, CELL_W, CHAIN_DX};
use crate::engine::geometry::{self, chain as chain_layout};
use crate::engine::graph::VizGraph;
use leptos::prelude::*;

use super::{arrow_defs, cursor_stack, diff_class, fitted_text, top_margin};

#[must_use]
pub fn chain(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let (union_nodes, union_edges) = geometry::union(graph);
    let layout = chain_layout::chain(&union_nodes, &union_edges).result;
    let half_w = CELL_W / 2.0;
    let half_h = CELL_H / 2.0;
    let margin = top_margin(graph, &layout, half_h + 8.0);
    let view_box = format!("0 {} {} {}", -margin, layout.width, layout.height + margin);
    let max_x = layout
        .positions
        .values()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let null_x = if max_x.is_finite() {
        max_x + CHAIN_DX * 0.55
    } else {
        0.0
    };
    let mid_y = layout.positions.values().map(|p| p.y).next().unwrap_or(0.0);
    let graph = graph.clone();
    view! {
        <svg class="viz-svg viz-chain" viewBox=view_box
             width=layout.width height=layout.height + margin>
            {arrow_defs()}
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                let edges: Vec<_> = step
                    .edges
                    .iter()
                    .filter_map(|e| {
                        let a = layout.positions.get(&e.from)?;
                        let b = layout.positions.get(&e.to)?;
                        let prev = e.label == "prev";
                        let yo = if prev { 8.0 } else { -8.0 };
                        let (x1, x2) = if a.x <= b.x {
                            (a.x + half_w, b.x - half_w)
                        } else {
                            (a.x - half_w, b.x + half_w)
                        };
                        let class = if prev { "viz-edge viz-edge--prev" } else { "viz-edge viz-edge--next" };
                        Some(view! {
                            <line class=class x1=x1 y1=a.y + yo x2=x2 y2=b.y + yo
                                  marker-end="url(#viz-arrow)"></line>
                        })
                    })
                    .collect();
                let nodes: Vec<_> = step
                    .nodes
                    .iter()
                    .filter_map(|n| {
                        let p = *layout.positions.get(&n.id)?;
                        let class = diff_class(step, &n.id, "viz-node");
                        let cursors: Vec<_> =
                            step.cursor.iter().filter(|c| c.target == n.id).cloned().collect();
                        Some(view! {
                            <g class=class>
                                <rect x=p.x - half_w y=p.y - half_h width=CELL_W height=CELL_H rx="6"></rect>
                                {fitted_text(&n.label, p.x, p.y, CELL_W - 8.0, 8.7)}
                                {cursor_stack(&cursors, p, half_h + 8.0)}
                            </g>
                        })
                    })
                    .collect();
                let null = (!step.nodes.is_empty()).then(|| view! {
                    <text class="viz-chain__null" x=null_x y=mid_y
                          text-anchor="middle" dominant-baseline="central">"∅"</text>
                });
                view! {
                    <g class="viz-chain__edges">{edges}</g>
                    <g class="viz-chain__nodes">{nodes}</g>
                    <g class="viz-chain__null">{null}</g>
                }
            }}
        </svg>
    }
    .into_any()
}
