//! The Tree family: circles of radius `NODE_R` on the
//! subtree-width layout, edges drawn behind, cursor stacks above.

use crate::engine::geometry::constants::NODE_R;
use crate::engine::geometry::{self, tree as tree_layout};
use crate::engine::graph::VizGraph;
use leptos::prelude::*;

use super::{cursor_stack, diff_class, fitted_text, top_margin};

#[must_use]
pub fn tree(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let (union_nodes, union_edges) = geometry::union(graph);
    let layout = tree_layout::tree(&union_nodes, &union_edges);
    let margin = top_margin(graph, &layout, NODE_R + 6.0);
    let view_box = format!("0 {} {} {}", -margin, layout.width, layout.height + margin);
    let graph = graph.clone();
    view! {
        <svg class="viz-svg viz-tree" viewBox=view_box
             width=layout.width height=layout.height + margin>
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                let edges: Vec<_> = step
                    .edges
                    .iter()
                    .filter_map(|e| {
                        let a = layout.positions.get(&e.from)?;
                        let b = layout.positions.get(&e.to)?;
                        Some(view! {
                            <line class="viz-edge" x1=a.x y1=a.y x2=b.x y2=b.y></line>
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
                                <circle cx=p.x cy=p.y r=NODE_R></circle>
                                {fitted_text(&n.label, p.x, p.y, 2.0 * NODE_R - 6.0, 8.7)}
                                {cursor_stack(&cursors, p, NODE_R + 6.0)}
                            </g>
                        })
                    })
                    .collect();
                view! {
                    <g class="viz-tree__edges">{edges}</g>
                    <g class="viz-tree__nodes">{nodes}</g>
                }
            }}
        </svg>
    }
    .into_any()
}
