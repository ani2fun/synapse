//! The placed linked list (`layoutHint: "placed"`): the author gives every node its place
//! (`slot` = column, an optional `row` meta field, default 0) and the drawing follows each
//! node's real pointers, so a frame can show what a chain-order layout cannot: a new node
//! waiting above the row, a next that skips a node, a reversed arrow, a cycle, a pointer
//! that has run off the end onto `∅`.
//!
//! Unlike the other families a node MAY move between steps (an inserted node drops into the
//! row): the place is authored per step, so the layout is computed per step. Only the
//! figure's extent is taken over the union of steps, so the viewBox never changes.
//!
//! Pointers:
//! - a node with no `next` edge has `next = null`: a short arrow to a `∅` after it;
//! - in a doubly list (any `prev` edge in any step) a node with no `prev` edge gets a `∅`
//!   before it;
//! - a cursor whose target is `null` sits on the `∅` after the last node of row 0 (a
//!   standalone `∅` when there is none, the empty list); `null:<id>` names the node whose
//!   `∅` it is.

use std::collections::HashMap;

use super::constants::{LIST_ARC, LIST_BOX_H, LIST_DY, LIST_FIELD_W, LIST_GAP, LIST_VAL_W, PAD};
use super::{LayoutResult, Point};
use crate::engine::graph::{NodeId, VizCursor, VizGraph, VizNode, VizStep};

/// The cursor target that means "this pointer holds null".
pub const NULL_TARGET: &str = "null";

/// How far the `∅` glyph sits beyond a box edge, and where its short arrow stops.
const NULL_DX: f64 = 28.0;
const NULL_ARROW: f64 = 17.0;
/// The two lanes of a pair of opposite arrows between the same two boxes.
const LANE: f64 = 7.0;

#[must_use]
pub fn is_next(label: &str) -> bool {
    matches!(label, "next" | "nxt")
}

#[must_use]
pub fn is_prev(label: &str) -> bool {
    matches!(label, "prev" | "previous")
}

fn row_of(n: &VizNode) -> i32 {
    n.meta
        .iter()
        .find(|f| f.name == "row")
        .and_then(|f| f.value.trim().parse().ok())
        .unwrap_or(0)
}

/// The column: `slot`, or a `col` meta field when a node sits between two columns (a node
/// about to be inserted, drawn over the gap it will fill).
fn col_of(n: &VizNode) -> f64 {
    n.meta
        .iter()
        .find(|f| f.name == "col")
        .and_then(|f| f.value.trim().parse().ok())
        .unwrap_or_else(|| f64::from(n.slot.unwrap_or(0)))
}

/// One node box at its centre.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedBox {
    pub node: VizNode,
    pub at: Point,
}

/// One pointer arrow as an SVG path; `prev` arrows are drawn dashed.
#[derive(Debug, Clone, PartialEq)]
pub struct Arrow {
    pub d: String,
    pub prev: bool,
}

/// Pointer names stacked over one point, `rise` above it.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorStack {
    pub at: Point,
    pub rise: f64,
    pub cursors: Vec<VizCursor>,
}

/// Everything one step draws.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frame {
    pub boxes: Vec<PlacedBox>,
    pub arrows: Vec<Arrow>,
    pub nulls: Vec<Point>,
    pub cursors: Vec<CursorStack>,
    /// An arc runs above the top row (a skip) or below the bottom row (a cycle).
    pub arc_above: bool,
    pub arc_below: bool,
}

/// The figure-wide frame of reference: where column/row 0 sit and the size of the drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub doubly: bool,
    pub width: f64,
    pub height: f64,
    x0: f64,
    y0: f64,
    min_col: f64,
    min_row: i32,
    centres: Vec<Point>,
}

impl Placement {
    #[must_use]
    pub fn box_w(&self) -> f64 {
        if self.doubly {
            LIST_VAL_W + 2.0 * LIST_FIELD_W
        } else {
            LIST_VAL_W + LIST_FIELD_W
        }
    }

    fn dx(&self) -> f64 {
        self.box_w() + LIST_GAP
    }

    #[allow(clippy::cast_precision_loss)]
    fn centre(&self, n: &VizNode) -> Point {
        Point {
            x: self.x0 + (col_of(n) - self.min_col) * self.dx(),
            y: self.y0 + f64::from(row_of(n) - self.min_row) * LIST_DY,
        }
    }

    /// Every centre any step uses — what the shared `top_margin` measures cursor headroom by.
    #[must_use]
    pub fn layout(&self) -> LayoutResult {
        LayoutResult {
            positions: self
                .centres
                .iter()
                .enumerate()
                .map(|(i, p)| (NodeId::new(format!("#{i}")), *p))
                .collect(),
            width: self.width,
            height: self.height,
        }
    }

    /// One step: its boxes, its arrows (each routed by where its target sits), the `∅`
    /// terminators and the cursor stacks.
    #[must_use]
    pub fn frame(&self, step: &VizStep) -> Frame {
        let at: Spots = step
            .nodes
            .iter()
            .map(|n| (n.id.value(), (self.centre(n), n)))
            .collect();
        let (mut arrows, arc_above, arc_below) = self.pointer_arrows(step, &at);
        let (ends, mut nulls, null_of) = self.null_ends(step, &at);
        arrows.extend(ends);
        let cursors = self.cursor_stacks(step, &at, &null_of, &mut nulls);
        Frame {
            boxes: step
                .nodes
                .iter()
                .map(|n| PlacedBox {
                    node: n.clone(),
                    at: at[n.id.value()].0,
                })
                .collect(),
            arrows,
            nulls,
            cursors,
            arc_above,
            arc_below,
        }
    }

    fn next_dot(&self, p: Point) -> f64 {
        p.x + self.box_w() / 2.0 - LIST_FIELD_W / 2.0
    }

    fn prev_dot(&self, p: Point) -> f64 {
        p.x - self.box_w() / 2.0 + LIST_FIELD_W / 2.0
    }

    /// Every next/prev edge as an arrow, plus whether any of them arcs above or below.
    fn pointer_arrows(&self, step: &VizStep, at: &Spots) -> (Vec<Arrow>, bool, bool) {
        let hw = self.box_w() / 2.0;
        let hh = LIST_BOX_H / 2.0;
        let has = |from: &str, to: &str, prev: bool| {
            step.edges.iter().any(|e| {
                e.from.value() == from
                    && e.to.value() == to
                    && if prev {
                        is_prev(&e.label)
                    } else {
                        is_next(&e.label)
                    }
            })
        };
        // Is there a node strictly between a and b on their (shared) row?
        let blocked = |a: Point, b: Point| {
            let (lo, hi) = if a.x < b.x { (a.x, b.x) } else { (b.x, a.x) };
            at.values()
                .any(|(p, _)| (p.y - a.y).abs() < f64::EPSILON && p.x > lo + 1.0 && p.x < hi - 1.0)
        };
        let mut arrows = Vec::new();
        let (mut arc_above, mut arc_below) = (false, false);
        for e in &step.edges {
            let prev = is_prev(&e.label);
            if !prev && !is_next(&e.label) {
                continue;
            }
            let (Some(&(a, _)), Some(&(b, _))) = (at.get(e.from.value()), at.get(e.to.value())) else {
                continue;
            };
            let dot = if prev { self.prev_dot(a) } else { self.next_dot(a) };
            let d = if (a.y - b.y).abs() > f64::EPSILON {
                // Another row: straight from the pointer field to the target's nearest edge.
                let end = clip(b, Point { x: dot, y: a.y }, hw, hh);
                format!("M{:.1},{:.1} L{:.1},{:.1}", dot, a.y, end.x, end.y)
            } else if (a.x - b.x).abs() < f64::EPSILON {
                // A node pointing at itself: a loop over its top.
                arc_above = true;
                format!(
                    "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
                    dot,
                    a.y - hh,
                    dot + 18.0,
                    a.y - hh - LIST_ARC,
                    a.x - 10.0,
                    a.y - hh - LIST_ARC,
                    a.x - 10.0,
                    a.y - hh
                )
            } else if !blocked(a, b) {
                // Neighbours on the row: one straight arrow; opposite arrows take two lanes
                // (next above, prev below in a doubly list).
                let right = b.x > a.x;
                let lane = if self.doubly {
                    if prev { LANE } else { -LANE }
                } else if has(e.to.value(), e.from.value(), prev) {
                    if right { -LANE } else { LANE }
                } else {
                    0.0
                };
                let x1 = if right {
                    if prev { a.x + hw } else { dot }
                } else if prev {
                    dot
                } else {
                    a.x - hw
                };
                let x2 = if right { b.x - hw } else { b.x + hw };
                format!("M{:.1},{:.1} L{:.1},{:.1}", x1, a.y + lane, x2, a.y + lane)
            } else if b.x > a.x {
                arc_above = true;
                // Skipping nodes forward: an arc over them, into the target's top.
                let (sy, ty) = (a.y - hh, b.y - hh);
                format!(
                    "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
                    dot,
                    sy,
                    dot,
                    sy - LIST_ARC,
                    b.x - 8.0,
                    ty - LIST_ARC,
                    b.x - 8.0,
                    ty
                )
            } else {
                arc_below = true;
                // Back past other nodes (a cycle): an arc under them, into the target's bottom.
                let (sy, ty) = (a.y + hh, b.y + hh);
                format!(
                    "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
                    dot,
                    sy,
                    dot,
                    sy + LIST_ARC,
                    b.x + 8.0,
                    ty + LIST_ARC,
                    b.x + 8.0,
                    ty
                )
            };
            arrows.push(Arrow { d, prev });
        }

        (arrows, arc_above, arc_below)
    }

    /// The `∅` ends: their short arrows, the glyphs, and which node owns each next `∅`.
    fn null_ends<'a>(
        &self,
        step: &'a VizStep,
        at: &Spots,
    ) -> (Vec<Arrow>, Vec<Point>, HashMap<&'a str, Point>) {
        let hw = self.box_w() / 2.0;
        let mut arrows = Vec::new();
        // `∅` after every null next (and before every null prev when doubly), keyed by owner.
        let mut nulls = Vec::new();
        let mut null_of: HashMap<&str, Point> = HashMap::new();
        for n in &step.nodes {
            let (p, _) = at[n.id.value()];
            let lane = if self.doubly { -LANE } else { 0.0 };
            if !step.edges.iter().any(|e| e.from == n.id && is_next(&e.label)) {
                let g = Point {
                    x: p.x + hw + NULL_DX,
                    y: p.y + lane,
                };
                arrows.push(Arrow {
                    d: format!(
                        "M{:.1},{:.1} L{:.1},{:.1}",
                        self.next_dot(p),
                        g.y,
                        p.x + hw + NULL_ARROW,
                        g.y
                    ),
                    prev: false,
                });
                nulls.push(g);
                null_of.insert(n.id.value(), g);
            }
            if self.doubly && !step.edges.iter().any(|e| e.from == n.id && is_prev(&e.label)) {
                let g = Point {
                    x: p.x - hw - NULL_DX,
                    y: p.y + LANE,
                };
                arrows.push(Arrow {
                    d: format!(
                        "M{:.1},{:.1} L{:.1},{:.1}",
                        self.prev_dot(p),
                        g.y,
                        p.x - hw - NULL_ARROW,
                        g.y
                    ),
                    prev: true,
                });
                nulls.push(g);
            }
        }

        (arrows, nulls, null_of)
    }

    /// Cursors grouped per box or per `∅`; a null cursor with no `∅` to sit on (the empty
    /// list) adds one.
    fn cursor_stacks(
        &self,
        step: &VizStep,
        at: &Spots,
        null_of: &HashMap<&str, Point>,
        nulls: &mut Vec<Point>,
    ) -> Vec<CursorStack> {
        let hh = LIST_BOX_H / 2.0;
        // Cursors: on a box, or on a `∅`.
        let mut on_box: Vec<(String, Vec<VizCursor>)> = Vec::new();
        let mut on_null: Vec<(Point, Vec<VizCursor>)> = Vec::new();
        for c in &step.cursor {
            let t = c.target.value();
            if let Some(owner) = t
                .strip_prefix("null:")
                .or_else(|| (t == NULL_TARGET).then_some(""))
            {
                let g = if owner.is_empty() {
                    last_null(step, null_of)
                } else {
                    null_of.get(owner).copied()
                };
                let g = g.unwrap_or_else(|| {
                    // The empty list: a `∅` of its own where the first node would be.
                    let g = Point {
                        x: self.x0 - self.min_col * self.dx(),
                        y: self.y0 + f64::from(-self.min_row) * LIST_DY,
                    };
                    nulls.push(g);
                    g
                });
                match on_null.iter_mut().find(|(p, _)| *p == g) {
                    Some((_, cs)) => cs.push(c.clone()),
                    None => on_null.push((g, vec![c.clone()])),
                }
            } else if at.contains_key(t) {
                match on_box.iter_mut().find(|(id, _)| id == t) {
                    Some((_, cs)) => cs.push(c.clone()),
                    None => on_box.push((t.to_owned(), vec![c.clone()])),
                }
            }
        }
        let mut cursors: Vec<CursorStack> = on_box
            .into_iter()
            .map(|(id, cs)| CursorStack {
                at: at[id.as_str()].0,
                rise: hh + 8.0,
                cursors: cs,
            })
            .collect();
        cursors.extend(on_null.into_iter().map(|(g, cs)| CursorStack {
            at: g,
            rise: 14.0,
            cursors: cs,
        }));
        nulls.dedup();

        cursors
    }
}

type Spots<'a> = HashMap<&'a str, (Point, &'a VizNode)>;

/// The `∅` that ends row 0's list: the rightmost row-0 node with a null next.
fn last_null(step: &VizStep, null_of: &HashMap<&str, Point>) -> Option<Point> {
    step.nodes
        .iter()
        .filter(|n| row_of(n) == 0)
        .filter_map(|n| null_of.get(n.id.value()).map(|g| (col_of(n), *g)))
        .max_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, g)| g)
}

/// Where the segment from a box's centre toward `from` leaves the box.
fn clip(c: Point, from: Point, hw: f64, hh: f64) -> Point {
    let (dx, dy) = (from.x - c.x, from.y - c.y);
    let sx = if dx.abs() > f64::EPSILON {
        hw / dx.abs()
    } else {
        f64::INFINITY
    };
    let sy = if dy.abs() > f64::EPSILON {
        hh / dy.abs()
    } else {
        f64::INFINITY
    };
    let s = sx.min(sy).min(1.0);
    Point {
        x: c.x + dx * s,
        y: c.y + dy * s,
    }
}

/// The frame of reference over every step: the extreme columns and rows, whether the list
/// is doubly, and room for the `∅` ends and the arcs.
#[must_use]
pub fn placement(graph: &VizGraph) -> Placement {
    let nodes: Vec<&VizNode> = graph.steps.iter().flat_map(|s| &s.nodes).collect();
    let doubly = graph
        .steps
        .iter()
        .flat_map(|s| &s.edges)
        .any(|e| is_prev(&e.label));
    let min_col = nodes.iter().map(|n| col_of(n)).fold(0.0, f64::min);
    let max_col = nodes.iter().map(|n| col_of(n)).fold(0.0, f64::max);
    let min_row = nodes.iter().map(|n| row_of(n)).min().unwrap_or(0).min(0);
    let max_row = nodes.iter().map(|n| row_of(n)).max().unwrap_or(0).max(0);
    let mut p = Placement {
        doubly,
        width: 0.0,
        height: 0.0,
        x0: 0.0,
        y0: 0.0,
        min_col,
        min_row,
        centres: Vec::new(),
    };
    let hw = p.box_w() / 2.0;
    let hh = LIST_BOX_H / 2.0;
    let end_room = NULL_DX + 10.0;
    p.x0 = PAD + hw + if doubly { end_room } else { 0.0 };
    // Arcs: room is only taken when some step draws one (the arc test does not depend on y0).
    let (mut above, mut below) = (false, false);
    for step in &graph.steps {
        let f = p.frame(step);
        above |= f.arc_above;
        below |= f.arc_below;
    }
    p.y0 = PAD + hh + if above { LIST_ARC } else { 0.0 };
    p.width = p.x0 + (max_col - min_col) * p.dx() + hw + end_room + PAD;
    p.height = p.y0 + f64::from(max_row - min_row) * LIST_DY + hh + if below { LIST_ARC } else { 0.0 } + PAD;
    p.centres = nodes.iter().map(|n| p.centre(n)).collect();
    if p.centres.is_empty() {
        p.centres.push(Point { x: p.x0, y: p.y0 });
    }
    p
}

#[cfg(test)]
mod tests;
