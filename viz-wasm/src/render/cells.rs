//! The Cells family (oracle: `CellRenderers`): the array row (index labels below, pointer
//! carets above) and the vertical LIFO stack (↑ TOP marker, cursors to the right). Layout
//! once over the union; the step signal redraws presence + diff classes only.

use crate::engine::geometry::constants::{CARET_ROW_H, CELL_DX, CELL_H, CELL_W, CURSOR_LINE_H, PAD};
use crate::engine::geometry::{self, linear};
use crate::engine::graph::VizGraph;
use leptos::prelude::*;

use super::{diff_class, fitted_text, themed};

#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn array(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let (union_nodes, union_edges) = geometry::union(graph);
    let layout = linear::array(&union_nodes, &union_edges);
    let max_stack = graph
        .steps
        .iter()
        .flat_map(|s| {
            let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
            for c in &s.cursor {
                *counts.entry(c.target.value()).or_default() += 1;
            }
            counts.into_values().collect::<Vec<_>>()
        })
        .max()
        .unwrap_or(0);
    let extra_lines = max_stack.saturating_sub(1);
    let top_margin = extra_lines as f64 * CURSOR_LINE_H + 1.0;
    let max_slot = union_nodes.iter().filter_map(|n| n.slot).max().unwrap_or(0);
    let view_box = format!(
        "0 {} {} {}",
        -top_margin,
        layout.width,
        layout.height + top_margin
    );

    // The index row is static (union-wide).
    let index_row: Vec<_> = (0..=max_slot)
        .map(|slot| {
            let x = PAD + f64::from(slot) * CELL_DX + CELL_W / 2.0;
            let y = CARET_ROW_H + CELL_H + 15.0;
            view! { <text x=x y=y text-anchor="middle">{slot.to_string()}</text> }
        })
        .collect();

    let graph = graph.clone();
    view! {
        <svg class="viz-svg viz-array" viewBox=view_box
             width=layout.width height=layout.height + top_margin>
            <g class="viz-array__index">{index_row}</g>
            <g class="viz-array__frame">
                {move || {
                    let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                    let cells: Vec<_> = step
                        .nodes
                        .iter()
                        .filter_map(|n| {
                            let p = layout.positions.get(&n.id)?;
                            let class = diff_class(step, &n.id, "viz-cell");
                            Some(view! {
                                <g class=class>
                                    <rect x=p.x y=p.y width=CELL_W height=CELL_H rx="6"></rect>
                                    {fitted_text(&n.label, p.x + CELL_W / 2.0, p.y + CELL_H / 2.0, CELL_W - 8.0, 9.3)}
                                </g>
                            })
                        })
                        .collect();
                    let carets: Vec<_> = {
                        // Group cursors by target so several names stack upward on one cell.
                        let mut by_target: std::collections::HashMap<&str, Vec<usize>> =
                            std::collections::HashMap::new();
                        for (i, c) in step.cursor.iter().enumerate() {
                            by_target.entry(c.target.value()).or_default().push(i);
                        }
                        step.cursor
                            .iter()
                            .enumerate()
                            .filter_map(|(i, c)| {
                                let p = layout.positions.get(&c.target)?;
                                let stack_pos = by_target[c.target.value()]
                                    .iter()
                                    .position(|&j| j == i)
                                    .unwrap_or(0);
                                let cx = p.x + CELL_W / 2.0;
                                let fill = themed(&c.color);
                                let name_y = CARET_ROW_H - 15.0 - stack_pos as f64 * CURSOR_LINE_H;
                                let name = c.name.clone();
                                Some(view! {
                                    <g class="viz-caret">
                                        <text class="viz-caret__name" x=cx y=name_y
                                              text-anchor="middle" fill=fill.clone()>{name}</text>
                                        {(stack_pos == 0).then(|| view! {
                                            <text x=cx y=CARET_ROW_H - 3.0 text-anchor="middle"
                                                  fill=fill>"▾"</text>
                                        })}
                                    </g>
                                })
                            })
                            .collect()
                    };
                    view! { {cells} {carets} }
                }}
            </g>
        </svg>
    }
    .into_any()
}

#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn stack(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let (union_nodes, union_edges) = geometry::union(graph);
    let layout = linear::stack(&union_nodes, &union_edges);
    let view_box = format!("0 0 {} {}", layout.width, layout.height);
    let top_y = layout
        .positions
        .values()
        .map(|p| p.y)
        .fold(f64::INFINITY, f64::min);
    let graph = graph.clone();
    view! {
        <svg class="viz-svg viz-stack" viewBox=view_box width=layout.width height=layout.height>
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                let live_top = step
                    .nodes
                    .iter()
                    .filter_map(|n| layout.positions.get(&n.id).map(|p| p.y))
                    .fold(f64::INFINITY, f64::min);
                let marker_y = if live_top.is_finite() { live_top } else { top_y };
                let cells: Vec<_> = step
                    .nodes
                    .iter()
                    .filter_map(|n| {
                        let p = layout.positions.get(&n.id)?;
                        let class = diff_class(step, &n.id, "viz-cell");
                        Some(view! {
                            <g class=class>
                                <rect x=p.x y=p.y width=CELL_W height=CELL_H rx="6"></rect>
                                {fitted_text(&n.label, p.x + CELL_W / 2.0, p.y + CELL_H / 2.0, CELL_W - 8.0, 9.3)}
                            </g>
                        })
                    })
                    .collect();
                let cursors: Vec<_> = step
                    .cursor
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        let p = layout.positions.get(&c.target)?;
                        let fill = themed(&c.color);
                        let y = p.y + CELL_H / 2.0 + (i as f64 - 0.5) * CURSOR_LINE_H;
                        let name = c.name.clone();
                        Some(view! {
                            <text class="viz-caret__name" x=p.x + CELL_W + 6.0 y=y fill=fill>{name}</text>
                        })
                    })
                    .collect();
                let marker = (!step.nodes.is_empty()).then(|| view! {
                    <text class="viz-stack__top" x=PAD + CELL_W / 2.0 y=marker_y - 8.0
                          text-anchor="middle">"↑ TOP"</text>
                });
                view! { {marker} {cells} {cursors} }
            }}
        </svg>
    }
    .into_any()
}
