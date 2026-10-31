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
//!
//! An arrow to another row is a curve that leaves the pointer field and enters the target
//! from one side, picked so that it crosses no other node. A node of kind `region` is not a
//! list node: it draws a labelled frame behind the list (meta `cols`, `rows`, `label`, and
//! `grid` for cell lines), so one figure can show, say, the nodes scattered in a memory grid
//! above the same list drawn logically. `layoutHint: "placed dy=72"` sets the row pitch.

use std::collections::HashMap;

use super::constants::{LIST_ARC, LIST_BOX_H, LIST_DY, LIST_FIELD_W, LIST_GAP, LIST_VAL_W, PAD};

/// The node kind that draws a labelled frame instead of a list node.
pub const REGION: &str = "region";
use super::{LayoutResult, Point};
use crate::engine::graph::{NodeId, VizCursor, VizGraph, VizNode, VizStep};

/// The cursor target that means "this pointer holds null".
pub const NULL_TARGET: &str = "null";

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
            nodes: step.nodes.iter().filter(|n| n.kind != REGION).cloned().collect(),
            ..step.clone()
        };
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

    fn next_dot(&self, p: Point) -> f64 {
        p.x + self.box_w() / 2.0 - LIST_FIELD_W / 2.0
    }

    fn prev_dot(&self, p: Point) -> f64 {
        p.x - self.box_w() / 2.0 + LIST_FIELD_W / 2.0
    }

    /// Every next/prev edge as an arrow, plus whether any of them arcs above or below.
    #[allow(clippy::too_many_lines)] // one match over the five shapes an arrow can take
    fn pointer_arrows(&self, step: &VizStep, at: &Spots) -> (Vec<Arrow>, bool, bool) {
        let hw = self.box_w() / 2.0;
        let hh = LIST_BOX_H / 2.0;
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
            if (a.y - b.y).abs() > f64::EPSILON {
                // Another row: a curve out of the pointer field and into one side of the
                // target, the shortest that crosses no other node.
                let mut others: Vec<Obstacle> = at
                    .iter()
                    .filter(|(id, _)| **id != e.from.value() && **id != e.to.value())
                    .map(|(_, (p, _))| Obstacle {
                        at: *p,
                        hw: hw + 6.0,
                        hh: hh + 6.0,
                    })
                    .collect();
                // A pointer's name stands above its node: an arrow must not run through it.
                others.extend(
                    step.cursor
                        .iter()
                        .filter_map(|c| at.get(c.target.value()))
                        .map(|(p, _)| Obstacle {
                            at: Point {
                                x: p.x,
                                y: p.y - hh - 18.0,
                            },
                            hw: 24.0,
                            hh: 16.0,
                        }),
                );
                arrows.push(self.cross_row(a, b, dot, &others, prev));
                continue;
            }
            let d = if (a.x - b.x).abs() < f64::EPSILON {
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
            } else if !blocked(a, b) && (b.x > a.x || self.doubly) {
                // Neighbours on the row: one straight arrow; a doubly list's opposite arrows take
                // two lanes (next above, prev below). A singly list's next to its left neighbour
                // is a turned link (a reversal in progress) and takes the back arc below instead,
                // clear of the `∅` that may sit in the gap.
                let right = b.x > a.x;
                let lane = if self.doubly {
                    if prev { LANE } else { -LANE }
                } else {
                    0.0
                };
                // From the box edge: the field's name fills the field, and the arrow must not
                // cross it.
                let x1 = if right { a.x + hw } else { a.x - hw };
                let x2 = if right { b.x - hw } else { b.x + hw };
                format!("M{:.1},{:.1} L{:.1},{:.1}", x1, a.y + lane, x2, a.y + lane)
            } else if b.x > a.x {
                // Skipping nodes forward: an arc over them into the target's top, or, when any
                // node it spans carries a pointer name (which stands above its node), an arc
                // under them into the target's bottom, where there are no names to cross.
                let named = step
                    .cursor
                    .iter()
                    .filter_map(|c| at.get(c.target.value()))
                    .any(|(p, _)| (p.y - a.y).abs() < f64::EPSILON && p.x > a.x - 1.0 && p.x < b.x + 1.0);
                let side = if named { 1.0 } else { -1.0 };
                let lift = LIST_ARC;
                if named {
                    arc_below = true;
                } else {
                    arc_above = true;
                }
                let (sy, ty) = (a.y + side * hh, b.y + side * hh);
                format!(
                    "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
                    dot,
                    sy,
                    dot,
                    sy + side * lift,
                    b.x - 8.0,
                    ty + side * lift,
                    b.x - 8.0,
                    ty
                )
            } else {
                arc_below = true;
                // Back past other nodes (a cycle), or back to the left neighbour: an arc under
                // them, into the target's bottom.
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
            let pts = path_points(&d);
            arrows.push(Arrow { d, prev, pts });
        }

        (arrows, arc_above, arc_below)
    }

    /// A cubic from the pointer field of `a` into `b`: it leaves the field down, up or
    /// sideways and enters `b` through its top, bottom or near side. The candidates are
    /// tried shortest first, and the first one that keeps clear of every other node wins.
    fn cross_row(&self, a: Point, b: Point, dot: f64, others: &[Obstacle], prev: bool) -> Arrow {
        let hw = self.box_w() / 2.0;
        let hh = LIST_BOX_H / 2.0;
        let reach = ((b.x - a.x).hypot(b.y - a.y) * 0.45).max(30.0);
        let side = if b.x >= a.x { 1.0 } else { -1.0 };
        let leaves = [
            (Point { x: dot, y: a.y + hh }, Point { x: 0.0, y: reach }),
            (Point { x: dot, y: a.y - hh }, Point { x: 0.0, y: -reach }),
            (
                Point {
                    x: a.x + side * hw,
                    y: a.y,
                },
                Point {
                    x: side * reach,
                    y: 0.0,
                },
            ),
        ];
        let enters = [
            (Point { x: b.x, y: b.y - hh }, Point { x: 0.0, y: -reach }),
            (Point { x: b.x, y: b.y + hh }, Point { x: 0.0, y: reach }),
            (
                Point {
                    x: b.x - side * hw,
                    y: b.y,
                },
                Point {
                    x: -side * reach,
                    y: 0.0,
                },
            ),
        ];
        let mut best: Option<(bool, f64, [Point; 4])> = None;
        for (start, out) in leaves {
            for (end, into) in enters {
                let curve = [
                    start,
                    Point {
                        x: start.x + out.x,
                        y: start.y + out.y,
                    },
                    Point {
                        x: end.x + into.x,
                        y: end.y + into.y,
                    },
                    end,
                ];
                let samples: Vec<Point> = (0..=32).map(|i| cubic(&curve, f64::from(i) / 32.0)).collect();
                let clear = !samples.iter().any(|q| {
                    others
                        .iter()
                        .any(|o| (q.x - o.at.x).abs() < o.hw && (q.y - o.at.y).abs() < o.hh)
                }) && !samples[3..30].iter().any(|q| {
                    [a, b]
                        .iter()
                        .any(|o| (q.x - o.x).abs() < hw - 1.0 && (q.y - o.y).abs() < hh - 1.0)
                });
                let len: f64 = samples
                    .windows(2)
                    .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
                    .sum();
                let better = match &best {
                    None => true,
                    Some((bc, bl, _)) => (clear && !*bc) || (clear == *bc && len < *bl),
                };
                if better {
                    best = Some((clear, len, curve));
                }
            }
        }
        let c = best.map_or([a, a, b, b], |(_, _, curve)| curve);
        Arrow {
            d: format!(
                "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
                c[0].x, c[0].y, c[1].x, c[1].y, c[2].x, c[2].y, c[3].x, c[3].y
            ),
            prev,
            pts: c.to_vec(),
        }
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
                        p.x + hw,
                        g.y,
                        p.x + hw + NULL_ARROW,
                        g.y
                    ),
                    prev: false,
                    pts: vec![g],
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
                        p.x - hw,
                        g.y,
                        p.x - hw - NULL_ARROW,
                        g.y
                    ),
                    prev: true,
                    pts: vec![g],
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

/// Something a curved arrow must keep clear of: a box around `at`, `hw` × `hh` half sizes.
#[derive(Debug, Clone, Copy)]
struct Obstacle {
    at: Point,
    hw: f64,
    hh: f64,
}

/// The `∅` that ends row 0's list: the rightmost row-0 node with a null next.
fn last_null(step: &VizStep, null_of: &HashMap<&str, Point>) -> Option<Point> {
    step.nodes
        .iter()
        .filter(|n| row_of(n).abs() < 1e-9)
        .filter_map(|n| null_of.get(n.id.value()).map(|g| (col_of(n), *g)))
        .max_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, g)| g)
}

fn cubic(c: &[Point; 4], t: f64) -> Point {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Point {
        x: w0 * c[0].x + w1 * c[1].x + w2 * c[2].x + w3 * c[3].x,
        y: w0 * c[0].y + w1 * c[1].y + w2 * c[2].y + w3 * c[3].y,
    }
}

/// Every coordinate pair in an SVG path string (end points and control points).
fn path_points(d: &str) -> Vec<Point> {
    d.split(|ch: char| ch.is_ascii_alphabetic() || ch == ' ')
        .filter_map(|xy| {
            let (x, y) = xy.split_once(',')?;
            Some(Point {
                x: x.parse().ok()?,
                y: y.parse().ok()?,
            })
        })
        .collect()
}

/// The frame of reference over every step: whether the list is doubly, the row pitch, and an
/// extent that holds every box, `∅`, arrow, region and region label of every step.
#[must_use]
pub fn placement(graph: &VizGraph) -> Placement {
    let nodes: Vec<&VizNode> = graph.steps.iter().flat_map(|s| &s.nodes).collect();
    let doubly = graph
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
            #[allow(clippy::cast_precision_loss)]
            let half = c
                .cursors
                .iter()
                .map(|x| x.name.chars().count())
                .max()
                .unwrap_or(0) as f64
                * 3.7
                + 4.0;
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
        .filter(|n| n.kind != REGION)
        .map(|n| p.centre(n))
        .collect();
    if p.centres.is_empty() {
        p.centres.push(Point { x: p.x0, y: p.y0 });
    }
    p
}

#[cfg(test)]
mod tests;
