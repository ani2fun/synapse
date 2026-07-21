//! The generic node/edge canvas — the Force family (seeded
//! deterministic layout) and Trie (tidy tree layout) share it: rim-trimmed arrowed edges
//! with optional labels, circle nodes with cursor rings + stacks.

use crate::engine::geometry::LayoutResult;
use crate::engine::geometry::constants::{NODE_R, RING_R};
use crate::engine::graph::VizGraph;
use leptos::prelude::*;

use super::{arrow_defs, cursor_stack, diff_class, fitted_text, themed, top_margin};

#[must_use]
pub fn canvas(graph: &VizGraph, layout: LayoutResult, step_index: Signal<usize>) -> AnyView {
    let margin = top_margin(graph, &layout, RING_R + 6.0);
    let view_box = format!("0 {} {} {}", -margin, layout.width, layout.height + margin);
    let graph = graph.clone();
    view! {
        <svg class="viz-svg viz-graph" viewBox=view_box
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
                        let dx = b.x - a.x;
                        let dy = b.y - a.y;
                        let len = (dx * dx + dy * dy).sqrt().max(1.0);
                        let (ux, uy) = (dx / len, dy / len);
                        let trim = NODE_R + 4.0;
                        let (x1, y1) = (a.x + ux * trim, a.y + uy * trim);
                        let (x2, y2) = (b.x - ux * trim, b.y - uy * trim);
                        // A label rides at the perpendicular-offset midpoint.
                        let label = (!e.label.is_empty()).then(|| {
                            let mx = f64::midpoint(x1, x2) - uy * 10.0;
                            let my = f64::midpoint(y1, y2) + ux * 10.0;
                            let text = e.label.clone();
                            view! {
                                <text class="viz-edge__label" x=mx y=my text-anchor="middle">{text}</text>
                            }
                        });
                        Some(view! {
                            <g class="viz-graph__edge">
                                <line class="viz-edge" x1=x1 y1=y1 x2=x2 y2=y2
                                      marker-end="url(#viz-arrow)"></line>
                                {label}
                            </g>
                        })
                    })
                    .collect();
                let nodes: Vec<_> = step
                    .nodes
                    .iter()
                    .filter_map(|n| {
                        let p = *layout.positions.get(&n.id)?;
                        let cursors: Vec<_> =
                            step.cursor.iter().filter(|c| c.target == n.id).cloned().collect();
                        let mut class = diff_class(step, &n.id, "viz-node");
                        if !cursors.is_empty() {
                            class.push_str(" viz-node--cursor");
                        }
                        // A ref placeholder showing its `key` meta reads as the key.
                        let key_meta = n.meta.iter().find(|f| f.name == "key").map(|f| f.value.clone());
                        let label = if n.label == "·" {
                            if let Some(k) = &key_meta {
                                class.push_str(" viz-node--key");
                                k.clone()
                            } else {
                                n.label.clone()
                            }
                        } else {
                            n.label.clone()
                        };
                        let ring = cursors.first().map(|c| {
                            let stroke = themed(&c.color);
                            view! {
                                <circle class="viz-node__ring" cx=p.x cy=p.y r=RING_R
                                        fill="none" stroke=stroke></circle>
                            }
                        });
                        Some(view! {
                            <g class=class>
                                {ring}
                                <circle cx=p.x cy=p.y r=NODE_R></circle>
                                {fitted_text(&label, p.x, p.y, 2.0 * NODE_R - 6.0, 8.7)}
                                {cursor_stack(&cursors, p, RING_R + 6.0)}
                            </g>
                        })
                    })
                    .collect();
                view! {
                    <g class="viz-graph__edges">{edges}</g>
                    <g class="viz-graph__nodes">{nodes}</g>
                }
            }}
        </svg>
    }
    .into_any()
}
