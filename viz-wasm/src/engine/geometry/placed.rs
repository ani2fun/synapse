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
//! - in a doubly list (any `prev` edge in any step, or `doubly` in the layout hint, for a
//!   figure whose only node is alone) a node with no `prev` edge gets a `∅` before it;
//! - a cursor whose target is `null` sits on the `∅` after the last node of row 0 (a
//!   standalone `∅` when there is none, the empty list); `null:<id>` names the node whose
//!   `∅` it is.
//!
//! An arrow to another row is a curve that leaves the pointer field and enters the target
//! from one side, picked so that it crosses no other node. A node of kind `region` is not a
//! list node: it draws a labelled frame behind the list (meta `cols`, `rows`, `label`, and
//! `grid` for cell lines), so one figure can show, say, the nodes scattered in a memory grid
//! above the same list drawn logically. `layoutHint: "placed dy=72"` sets the row pitch, and
//! `"placed doubly"` draws a doubly list whose steps have no `prev` edge yet.
//!
//! A next edge to `null:<id>` holds null, but its `∅` stands at another node's place (a reversal
//! whose first node now ends the list, with the `∅` still at the far right). The arrow runs over
//! the row, above the pointer names, and that `∅` is drawn even if `<id>` has a next of its own.
//!
//! Two more kinds are annotations: `gap-arrow` is a straight down arrow centred on its (`col`,
//! `row`) with meta `len` and an optional `label` beside it; `ring` draws a dashed ellipse around
//! the pointer name in its meta `name`.

use std::collections::HashMap;

use super::constants::{
    CURSOR_CARET_GAP, CURSOR_LINE_H, LIST_BOX_H, LIST_DY, LIST_FIELD_W, LIST_GAP, LIST_VAL_W, PAD,
};

mod arrows;

/// The node kind that draws a labelled frame instead of a list node.
pub const REGION: &str = "region";
/// The node kind that draws a down arrow between two drawings.
pub const GAP_ARROW: &str = "gap-arrow";
/// The node kind that rings a pointer's name.
pub const RING: &str = "ring";
use super::{LayoutResult, Point};
use crate::engine::graph::{NodeId, VizCursor, VizGraph, VizNode, VizStep};

/// The cursor target that means "this pointer holds null".
pub const NULL_TARGET: &str = "null";

/// Is this node an annotation rather than a list node?
fn is_mark(n: &VizNode) -> bool {
    matches!(n.kind.as_str(), REGION | GAP_ARROW | RING)
}

/// How far the `∅` glyph sits beyond a box edge, and where its short arrow stops.
const NULL_DX: f64 = 28.0;
const NULL_ARROW: f64 = 17.0;
/// The two lanes of a pair of opposite arrows between the same two boxes.
const LANE: f64 = 7.0;

/// Is this layout hint the placed list (`placed`, optionally with `dy=<pitch>`)?
#[must_use]
pub fn is_placed(hint: &str) -> bool {
    hint.split_whitespace().next() == Some("placed")
}

fn row_pitch(hint: &str) -> f64 {
    hint.split_whitespace()
        .filter_map(|t| t.strip_prefix("dy="))
        .find_map(|v| v.parse::<f64>().ok())
        .filter(|v| *v >= LIST_BOX_H + 8.0)
        .unwrap_or(LIST_DY)
}

fn meta_f(n: &VizNode, name: &str) -> Option<f64> {
    n.meta
        .iter()
        .find(|f| f.name == name)
        .and_then(|f| f.value.trim().parse().ok())
}

fn meta_s<'a>(n: &'a VizNode, name: &str) -> Option<&'a str> {
    n.meta.iter().find(|f| f.name == name).map(|f| f.value.as_str())
}

#[must_use]
pub fn is_next(label: &str) -> bool {
    matches!(label, "next" | "nxt")
}

#[must_use]
pub fn is_prev(label: &str) -> bool {
    matches!(label, "prev" | "previous")
}

/// The row: 0 is the list, -1 above it, 1 below; fractional to centre a node or a frame
/// between rows.
fn row_of(n: &VizNode) -> f64 {
    meta_f(n, "row").unwrap_or(0.0)
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

/// One pointer arrow as an SVG path; `prev` arrows are drawn dashed. `pts` are the path's
/// points and control points: the curve lies inside their hull, which is what the extent uses.
#[derive(Debug, Clone, PartialEq)]
pub struct Arrow {
    pub d: String,
    pub prev: bool,
    pub pts: Vec<Point>,
}

/// A labelled frame behind the list; `lines` are its cell lines when it is a grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub label: String,
    pub lines: Vec<(Point, Point)>,
}

/// How far below its frame a region's label sits, and the room it takes.
pub const REGION_LABEL_DY: f64 = 18.0;

/// A straight down arrow between two drawings, with an optional label to its right.
#[derive(Debug, Clone, PartialEq)]
pub struct GapArrow {
    pub x: f64,
    pub y0: f64,
    pub y1: f64,
    pub label: String,
}

/// How far right of a gap arrow its label starts.
pub const GAP_LABEL_DX: f64 = 14.0;

/// A dashed ellipse around a pointer's name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ring {
    pub x: f64,
    pub y: f64,
    pub rx: f64,
    pub ry: f64,
}

/// Half the drawn width of a pointer name.
#[allow(clippy::cast_precision_loss)] // pointer names are short
fn name_half(name: &str) -> f64 {
    name.chars().count() as f64 * 3.7 + 4.0
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
    pub regions: Vec<Region>,
    pub gaps: Vec<GapArrow>,
    pub rings: Vec<Ring>,
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
    min_row: f64,
    dy: f64,
    /// The most pointer names any step stacks on one spot: an arrow over the row clears them.
    stack_max: usize,
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
            y: self.y0 + (row_of(n) - self.min_row) * self.dy,
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
        let regions = step
            .nodes
            .iter()
            .filter(|n| n.kind == REGION)
            .map(|n| self.region(n))
            .collect();
        let list = VizStep {
            nodes: step.nodes.iter().filter(|n| !is_mark(n)).cloned().collect(),
            ..step.clone()
        };
        let gaps = step
            .nodes
            .iter()
            .filter(|n| n.kind == GAP_ARROW)
            .map(|n| {
                let c = self.centre(n);
                let len = meta_f(n, "len").unwrap_or(40.0);
                GapArrow {
                    x: c.x,
                    y0: c.y - len / 2.0,
                    y1: c.y + len / 2.0,
                    label: meta_s(n, "label").unwrap_or_default().to_owned(),
                }
            })
            .collect();
        let rung: Vec<&str> = step
            .nodes
            .iter()
            .filter(|n| n.kind == RING)
            .filter_map(|n| meta_s(n, "name"))
            .collect();
        let step = &list;
        let at: Spots = step
            .nodes
            .iter()
            .map(|n| (n.id.value(), (self.centre(n), n)))
            .collect();
        let (mut arrows, arc_above, arc_below) = self.pointer_arrows(step, &at);
        let (ends, mut nulls, null_of) = self.null_ends(step, &at);
        arrows.extend(ends);
        let cursors = self.cursor_stacks(step, &at, &null_of, &mut nulls);
        let rings = rings_of(&cursors, &rung);
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
            regions,
            gaps,
            rings,
            arc_above,
            arc_below,
        }
    }

    /// A region's frame: from its top-left cell (`col`/`slot`, `row`) over `cols` × `rows`
    /// cells of the list's own pitch, so a node placed in a cell sits in the middle of it.
    fn region(&self, n: &VizNode) -> Region {
        let cols = meta_f(n, "cols").unwrap_or(1.0);
        let rows = meta_f(n, "rows").unwrap_or(1.0);
        let corner = self.centre(n);
        let left = corner.x - self.dx() / 2.0;
        let top = corner.y - self.dy / 2.0;
        let width = cols * self.dx();
        let height = rows * self.dy;
        let mut lines = Vec::new();
        if meta_s(n, "grid") == Some("true") {
            let mut cell = 1.0;
            while cell < cols - 1e-9 {
                let gx = left + cell * self.dx();
                lines.push((
                    Point { x: gx, y: top },
                    Point {
                        x: gx,
                        y: top + height,
                    },
                ));
                cell += 1.0;
            }
            let mut cell = 1.0;
            while cell < rows - 1e-9 {
                let gy = top + cell * self.dy;
                lines.push((
                    Point { x: left, y: gy },
                    Point {
                        x: left + width,
                        y: gy,
                    },
                ));
                cell += 1.0;
            }
        }
        Region {
            x: left,
            y: top,
            w: width,
            h: height,
            label: meta_s(n, "label").unwrap_or_default().to_owned(),
            lines,
        }
    }

    /// Where the `∅` after a node stands.
    fn null_spot(&self, p: Point) -> Point {
        Point {
            x: p.x + self.box_w() / 2.0 + NULL_DX,
            y: p.y,
        }
    }

    /// The `∅` ends: their short arrows, the glyphs, and which node owns each `∅` (its next
    /// `∅`, or its prev `∅` when its next is not null).
    fn null_ends<'a>(
        &self,
        step: &'a VizStep,
        at: &Spots,
    ) -> (Vec<Arrow>, Vec<Point>, HashMap<&'a str, Point>) {
        let hw = self.box_w() / 2.0;
        let mut arrows = Vec::new();
        // `∅` after every null next (and before every null prev when doubly), keyed by owner. A
        // doubly linked node turned round (its prev pointing right or its next pointing left, as
        // in a reversal) gets its `∅` on the other side, where that pointer now leads.
        let mut nulls = Vec::new();
        let mut null_of: HashMap<&str, Point> = HashMap::new();
        for n in &step.nodes {
            let (p, _) = at[n.id.value()];
            let points_right = |kind: fn(&str) -> bool| {
                step.edges
                    .iter()
                    .find(|e| e.from == n.id && kind(&e.label))
                    .and_then(|e| at.get(e.to.value()))
                    .map(|(q, _)| q.x > p.x)
            };
            // An edge of `kind` coming into this node from the side `dir` points to: it holds that
            // side's lane in the gap, where a `∅` would sit on top of its arrowhead.
            let arrives_from = |kind: fn(&str) -> bool, dir: f64| {
                step.edges.iter().any(|e| {
                    e.to == n.id
                        && kind(&e.label)
                        && at
                            .get(e.from.value())
                            .is_some_and(|(q, _)| (q.x - p.x) * dir > 0.0 && (q.y - p.y).abs() < 1.0)
                })
            };
            // The `∅` lane on the side `dir`: `own`, unless an edge of `own_kind` comes in there and
            // the other lane is free of `other_kind` edges.
            let lane_at = |dir: f64, own: f64, own_kind: fn(&str) -> bool, other_kind: fn(&str) -> bool| {
                if arrives_from(own_kind, dir) && !arrives_from(other_kind, dir) {
                    -own
                } else {
                    own
                }
            };
            let mut end = |dir: f64, lane: f64, prev: bool| {
                let g = Point {
                    x: p.x + dir * (hw + NULL_DX),
                    y: p.y + lane,
                };
                arrows.push(Arrow {
                    d: format!(
                        "M{:.1},{:.1} L{:.1},{:.1}",
                        p.x + dir * hw,
                        g.y,
                        p.x + dir * (hw + NULL_ARROW),
                        g.y
                    ),
                    prev,
                    pts: vec![g],
                });
                nulls.push(g);
                g
            };
            if !step.edges.iter().any(|e| e.from == n.id && is_next(&e.label)) {
                let (dir, lane) = if !self.doubly {
                    (1.0, 0.0)
                } else if points_right(is_prev) == Some(true) {
                    (-1.0, lane_at(-1.0, -LANE, is_next, is_prev))
                } else {
                    (1.0, -LANE)
                };
                null_of.insert(n.id.value(), end(dir, lane, false));
            }
            if self.doubly && !step.edges.iter().any(|e| e.from == n.id && is_prev(&e.label)) {
                let (dir, lane) = if points_right(is_next) == Some(false) {
                    (1.0, lane_at(1.0, LANE, is_prev, is_next))
                } else {
                    (-1.0, LANE)
                };
                let g = end(dir, lane, true);
                null_of.entry(n.id.value()).or_insert(g);
            }
        }
        // A `∅` an arrow from over the row ends at, where its owner has no `∅` of its own.
        for e in &step.edges {
            if let Some(owner) = e.to.value().strip_prefix("null:")
                && let Some(&(b, _)) = at.get(owner)
                && !null_of.contains_key(owner)
            {
                let g = self.null_spot(b);
                nulls.push(g);
                null_of.insert(owner, g);
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
                        y: self.y0 - self.min_row * self.dy,
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
        .filter(|n| row_of(n).abs() < 1e-9)
        .filter_map(|n| null_of.get(n.id.value()).map(|g| (col_of(n), *g)))
        .max_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, g)| g)
}

/// The ring of each named pointer that has a stack in this frame.
fn rings_of(stacks: &[CursorStack], names: &[&str]) -> Vec<Ring> {
    let mut rings = Vec::new();
    for s in stacks {
        for (i, c) in s.cursors.iter().enumerate() {
            if names.contains(&c.name.as_str()) {
                #[allow(clippy::cast_precision_loss)] // cursor stacks are tiny
                let baseline = s.at.y - (s.rise + CURSOR_CARET_GAP + i as f64 * CURSOR_LINE_H);
                rings.push(Ring {
                    x: s.at.x,
                    y: baseline - 4.0,
                    rx: name_half(&c.name) + 9.0,
                    ry: 13.0,
                });
            }
        }
    }
    rings
}

/// The frame of reference over every step: whether the list is doubly, the row pitch, and an
/// extent that holds every box, `∅`, arrow, region and region label of every step.
#[must_use]
#[allow(clippy::too_many_lines)] // one measuring pass over everything a frame can draw
pub fn placement(graph: &VizGraph) -> Placement {
    let nodes: Vec<&VizNode> = graph.steps.iter().flat_map(|s| &s.nodes).collect();
    let doubly = graph.layout_hint.split_whitespace().any(|t| t == "doubly")
        || graph
            .steps
            .iter()
            .flat_map(|s| &s.edges)
            .any(|e| is_prev(&e.label));
    let mut p = Placement {
        doubly,
        width: 0.0,
        height: 0.0,
        x0: 0.0,
        y0: 0.0,
        min_col: nodes.iter().map(|n| col_of(n)).fold(0.0, f64::min),
        min_row: nodes.iter().map(|n| row_of(n)).fold(0.0, f64::min),
        dy: row_pitch(&graph.layout_hint),
        stack_max: graph
            .steps
            .iter()
            .flat_map(|s| {
                let mut counts: HashMap<&str, usize> = HashMap::new();
                for c in &s.cursor {
                    *counts.entry(c.target.value()).or_default() += 1;
                }
                counts.into_values().collect::<Vec<_>>()
            })
            .max()
            .unwrap_or(0),
        centres: Vec::new(),
    };
    let hw = p.box_w() / 2.0;
    let hh = LIST_BOX_H / 2.0;
    // Lay every step out from a provisional origin, measure, then shift the origin so the
    // whole drawing starts PAD in from the top-left corner.
    let (mut lo, mut hi) = (
        Point {
            x: f64::INFINITY,
            y: f64::INFINITY,
        },
        Point {
            x: f64::NEG_INFINITY,
            y: f64::NEG_INFINITY,
        },
    );
    let mut grow = |x0: f64, y0: f64, x1: f64, y1: f64| {
        lo = Point {
            x: lo.x.min(x0),
            y: lo.y.min(y0),
        };
        hi = Point {
            x: hi.x.max(x1),
            y: hi.y.max(y1),
        };
    };
    for step in &graph.steps {
        let f = p.frame(step);
        for b in &f.boxes {
            grow(b.at.x - hw, b.at.y - hh, b.at.x + hw, b.at.y + hh);
        }
        // Pointer names are centred over their node and can be wider than it.
        for c in &f.cursors {
            let half = c.cursors.iter().map(|x| name_half(&x.name)).fold(4.0, f64::max);
            grow(c.at.x - half, c.at.y, c.at.x + half, c.at.y);
        }
        for g in &f.nulls {
            grow(g.x - 10.0, g.y - 10.0, g.x + 10.0, g.y + 10.0);
        }
        for a in &f.arrows {
            for q in &a.pts {
                grow(q.x, q.y, q.x, q.y);
            }
        }
        for r in &f.regions {
            let label = if r.label.is_empty() {
                0.0
            } else {
                REGION_LABEL_DY + 6.0
            };
            grow(r.x, r.y, r.x + r.w, r.y + r.h + label);
        }
        for g in &f.gaps {
            #[allow(clippy::cast_precision_loss)] // labels are short
            let text = g.label.chars().count() as f64 * 8.0;
            grow(g.x - 6.0, g.y0, g.x + GAP_LABEL_DX + text, g.y1);
        }
        for r in &f.rings {
            grow(
                r.x - r.rx - 2.0,
                r.y - r.ry - 2.0,
                r.x + r.rx + 2.0,
                r.y + r.ry + 2.0,
            );
        }
    }
    if !lo.x.is_finite() {
        (lo, hi) = (Point { x: -10.0, y: -10.0 }, Point { x: 10.0, y: 10.0 });
    }
    p.x0 = PAD - lo.x;
    p.y0 = PAD - lo.y;
    p.width = hi.x - lo.x + 2.0 * PAD;
    p.height = hi.y - lo.y + 2.0 * PAD;
    p.centres = nodes
        .iter()
        .filter(|n| !is_mark(n))
        .map(|n| p.centre(n))
        .collect();
    if p.centres.is_empty() {
        p.centres.push(Point { x: p.x0, y: p.y0 });
    }
    p
}

#[cfg(test)]
mod tests;
