//! The render kit (oracle: `RenderKit.scala`) — the SVG shared bits every family uses: diff
//! classes, the themed role-colour mapping, cursor stacking, top-margin headroom, and
//! fitted text.

pub mod buckets;
pub mod cells;
pub mod chain;
pub mod dom;
pub mod dual;
pub mod graph_canvas;
pub mod grid_table;
pub mod list_chain;
pub mod strip;
pub mod tree;

use crate::engine::geometry::constants::{CURSOR_GLYPH_UP, CURSOR_LINE_H};
use crate::engine::geometry::{LayoutResult, Point};
use crate::engine::graph::{NodeId, VizCursor, VizGraph, VizStep};
use leptos::prelude::*;

/// Gap between the caret glyph and the first stacked name (oracle: `CursorCaretGap`).
pub const CURSOR_CARET_GAP: f64 = 12.0;

/// The diff-cue class for a node id in a step (`--new` / `--changed` / `--removed`).
#[must_use]
pub fn diff_class(step: &VizStep, id: &NodeId, base: &str) -> String {
    if step.removed.contains(id) {
        format!("{base} {base}--removed")
    } else if step.changed.contains(id) {
        format!("{base} {base}--changed")
    } else if step.highlight.contains(id) {
        format!("{base} {base}--new")
    } else {
        base.to_owned()
    }
}

/// Wire hex → the theme-aware `--viz-role-*` token, hex fallback (RenderKit.themed).
#[must_use]
pub fn themed(hex: &str) -> String {
    let token = match hex {
        "#3a5a8c" => Some("anchor"),
        "#4f5bd5" => Some("active"),
        "#8a4f7d" => Some("trail"),
        "#5a8a5a" => Some("ahead"),
        "#a13e3e" => Some("end"),
        "#6a9656" => Some("changed"),
        "#c5a572" => Some("alt1"),
        "#91b5c2" => Some("alt2"),
        "" => None,
        _ => return hex.to_owned(),
    };
    token.map_or_else(
        || "currentColor".to_owned(),
        |t| format!("var(--viz-role-{t}, {hex})"),
    )
}

/// Cursors grouped onto one node stack UPWARD one line apart — never overlap: a single `▾`
/// caret at `base_rise`, then the names.
#[must_use]
pub fn cursor_stack(cursors: &[VizCursor], p: Point, base_rise: f64) -> impl IntoView + use<> {
    if cursors.is_empty() {
        return None;
    }
    let caret_color = themed(&cursors[0].color);
    let names: Vec<_> = cursors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            #[allow(clippy::cast_precision_loss)] // cursor stacks are tiny
            let y = p.y - (base_rise + CURSOR_CARET_GAP + i as f64 * CURSOR_LINE_H);
            let fill = themed(&c.color);
            let name = c.name.clone();
            view! {
                <text class="viz-node__cursor" x=p.x y=y text-anchor="middle" fill=fill>{name}</text>
            }
        })
        .collect();
    Some(view! {
        <g class="viz-cursors">
            <text class="viz-node__cursor-caret" x=p.x y=p.y - base_rise
                  text-anchor="middle" fill=caret_color>"▾"</text>
            {names}
        </g>
    })
}

/// Headroom above the topmost node so stacked cursor labels never clip (the "root label
/// truncated" bug): `cursor_rise − min centre y`, floored at 0.
#[must_use]
pub fn top_margin(graph: &VizGraph, layout: &LayoutResult, base_rise: f64) -> f64 {
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
    if max_stack == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let cursor_rise =
        base_rise + CURSOR_CARET_GAP + (max_stack.saturating_sub(1)) as f64 * CURSOR_LINE_H + CURSOR_GLYPH_UP;
    let min_centre_y = layout
        .positions
        .values()
        .map(|p| p.y)
        .fold(f64::INFINITY, f64::min);
    let min_centre_y = if min_centre_y.is_finite() {
        min_centre_y
    } else {
        0.0
    };
    (cursor_rise - min_centre_y).max(0.0)
}

/// Centre-anchored label that squeezes into `max_w` via `textLength` when it would overflow.
#[must_use]
pub fn fitted_text(label: &str, x: f64, y: f64, max_w: f64, char_w: f64) -> impl IntoView + use<> {
    #[allow(clippy::cast_precision_loss)]
    let natural = label.chars().count() as f64 * char_w;
    let squeeze = natural > max_w;
    let label = label.to_owned();
    view! {
        <text
            x=x
            y=y
            text-anchor="middle"
            dominant-baseline="central"
            textLength=squeeze.then(|| max_w.to_string())
            lengthAdjust=squeeze.then_some("spacingAndGlyphs")
        >
            {label}
        </text>
    }
}

/// The shared arrowhead marker defs.
#[must_use]
pub fn arrow_defs() -> impl IntoView {
    view! {
        <defs>
            <marker id="viz-arrow" viewBox="0 0 10 10" refX="9" refY="5"
                    markerWidth="7" markerHeight="7" orient="auto-start-reverse">
                <path class="viz-arrowhead" d="M0,0 L10,5 L0,10 z"></path>
            </marker>
        </defs>
    }
}
