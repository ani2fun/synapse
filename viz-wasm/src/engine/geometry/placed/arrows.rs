//! Routing a placed list's pointer arrows: a straight arrow between neighbours, an arc over or
//! under the nodes it skips, a curve to another row that keeps clear of every other node, and a
//! lane over the row to a far `∅`.

use super::{Arrow, LANE, Placement, Spots, is_next, is_prev};
use crate::engine::geometry::constants::{
    CURSOR_CARET_GAP, CURSOR_GLYPH_UP, CURSOR_LINE_H, LIST_ARC, LIST_BOX_H, LIST_FIELD_W,
};
use crate::engine::geometry::{Point, cubic, path_points};
use crate::engine::graph::VizStep;

impl Placement {
    fn next_dot(&self, p: Point) -> f64 {
        p.x + self.box_w() / 2.0 - LIST_FIELD_W / 2.0
    }

    fn prev_dot(&self, p: Point) -> f64 {
        p.x - self.box_w() / 2.0 + LIST_FIELD_W / 2.0
    }

    /// Every next/prev edge as an arrow, plus whether any of them arcs above or below.
    #[allow(clippy::too_many_lines)] // one match over the five shapes an arrow can take
    pub(super) fn pointer_arrows(&self, step: &VizStep, at: &Spots) -> (Vec<Arrow>, bool, bool) {
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
            if let Some(owner) = e.to.value().strip_prefix("null:") {
                if let (Some(&(a, _)), Some(&(b, _))) = (at.get(e.from.value()), at.get(owner)) {
                    arrows.push(self.over_to_null(a, self.null_spot(b), prev));
                }
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
                curve_d(&[
                    Point { x: dot, y: a.y - hh },
                    Point {
                        x: dot + 18.0,
                        y: a.y - hh - LIST_ARC,
                    },
                    Point {
                        x: a.x - 10.0,
                        y: a.y - hh - LIST_ARC,
                    },
                    Point {
                        x: a.x - 10.0,
                        y: a.y - hh,
                    },
                ])
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
                curve_d(&[
                    Point { x: dot, y: sy },
                    Point {
                        x: dot,
                        y: sy + side * lift,
                    },
                    Point {
                        x: b.x - 8.0,
                        y: ty + side * lift,
                    },
                    Point { x: b.x - 8.0, y: ty },
                ])
            } else {
                arc_below = true;
                // Back past other nodes (a cycle), or back to the left neighbour: an arc under
                // them, into the target's bottom.
                let (sy, ty) = (a.y + hh, b.y + hh);
                curve_d(&[
                    Point { x: dot, y: sy },
                    Point {
                        x: dot,
                        y: sy + LIST_ARC,
                    },
                    Point {
                        x: b.x + 8.0,
                        y: ty + LIST_ARC,
                    },
                    Point { x: b.x + 8.0, y: ty },
                ])
            };
            let pts = path_points(&d);
            arrows.push(Arrow { d, prev, pts });
        }

        (arrows, arc_above, arc_below)
    }

    /// An arrow from `a`'s right edge to a far `∅`: up through the gap beside `a`, along a lane
    /// above the tallest stack of pointer names, down past the `∅`, and back into it from the
    /// right, so it crosses no name, not even one standing over that `∅`.
    fn over_to_null(&self, a: Point, g: Point, prev: bool) -> Arrow {
        const RUN: f64 = 14.0;
        const R: f64 = 8.0;
        const PAST: f64 = 40.0;
        const CLEAR: f64 = 6.0;
        let hh = LIST_BOX_H / 2.0;
        let x0 = a.x + self.box_w() / 2.0;
        let up = x0 + RUN;
        #[allow(clippy::cast_precision_loss)] // cursor stacks are tiny
        let names = self.stack_max.saturating_sub(1) as f64 * CURSOR_LINE_H;
        let lane = a.y - (hh + 8.0 + CURSOR_CARET_GAP + names + CURSOR_GLYPH_UP + CLEAR);
        let rx = g.x + PAST;
        let d = format!(
            "M{x0:.1},{ay:.1} L{l1:.1},{ay:.1} Q{up:.1},{ay:.1} {up:.1},{u1:.1} L{up:.1},{u2:.1} \
             Q{up:.1},{lane:.1} {r1:.1},{lane:.1} L{r2:.1},{lane:.1} Q{rx:.1},{lane:.1} {rx:.1},{d1:.1} \
             L{rx:.1},{d2:.1} Q{rx:.1},{gy:.1} {r2:.1},{gy:.1} L{end:.1},{gy:.1}",
            ay = a.y,
            l1 = up - R,
            u1 = a.y - R,
            u2 = lane + R,
            r1 = up + R,
            r2 = rx - R,
            d1 = lane + R,
            d2 = g.y - R,
            gy = g.y,
            end = g.x + 10.0,
        );
        let pts = vec![
            Point { x: x0, y: a.y },
            Point { x: up, y: lane },
            Point { x: rx, y: lane },
            Point { x: rx, y: g.y },
        ];
        Arrow { d, prev, pts }
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
        let d = curve_d(&c);
        let pts = path_points(&d);
        Arrow { d, prev, pts }
    }
}

/// How far a curved arrow runs straight before its target. The head is about 10.5 px long
/// (`markerWidth` 7 at stroke 1.5) and takes the direction at the very end of the path, so the
/// line under the head has to run in that direction, or the two meet at a visible angle.
const HEAD_RUN: f64 = 12.0;

/// The path of a curved arrow: the cubic `c`, stopping `HEAD_RUN` short of its end, then a
/// straight run into the end along the last control point's direction, so the bend stays
/// smooth. A last control point nearer than that leaves no room, and the cubic stays whole.
fn curve_d(c: &[Point; 4]) -> String {
    let [start, c1, c2, end] = *c;
    let reach = (end.x - c2.x).hypot(end.y - c2.y);
    if reach <= HEAD_RUN {
        return format!(
            "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
            start.x, start.y, c1.x, c1.y, c2.x, c2.y, end.x, end.y
        );
    }
    let back = HEAD_RUN / reach;
    let (jx, jy) = (end.x + (c2.x - end.x) * back, end.y + (c2.y - end.y) * back);
    format!(
        "M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} L{:.1},{:.1}",
        start.x, start.y, c1.x, c1.y, c2.x, c2.y, jx, jy, end.x, end.y
    )
}

/// Something a curved arrow must keep clear of: a box around `at`, `hw` × `hh` half sizes.
#[derive(Debug, Clone, Copy)]
struct Obstacle {
    at: Point,
    hw: f64,
    hh: f64,
}
