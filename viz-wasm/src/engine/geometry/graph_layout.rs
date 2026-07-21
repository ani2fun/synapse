//! The Graph family: DUAL — a tree-shaped (acyclic, ≤1
//! parent) union lays out as a tidy forest under a synthetic super-root; a cyclic/merging
//! union falls to a SEEDED, DETERMINISTIC force simulation: velocity-Verlet with pinned
//! constants (link 100 · manyBody −520 · collide RingR+12 · x/y anchor 0.07 ·
//! 320 synchronous ticks · velocityDecay 0.6) and a Mulberry32(0x5eed) PRNG for the jiggle,
//! so a redraw is byte-identical. Pixel-parity with d3 is a NON-goal —
//! determinism + readable layout is.

use std::collections::{HashMap, HashSet};

use super::constants::{NODE_R, PAD, RING_R, TREE_ROW_H};
use super::{LayoutResult, Point, tree};
use crate::engine::graph::{NodeId, VizEdge, VizNode};

// Seeded PRNG, so `jiggle` (and therefore the whole sim) is reproducible.
struct Mulberry32(u32);

impl Mulberry32 {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(0x3d | t))) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

const SEED: u32 = 0x5eed;
const TICKS: u32 = 320;
const VELOCITY_DECAY: f64 = 0.6;
const LINK_DISTANCE: f64 = 100.0;
const MANY_BODY: f64 = -520.0;
const ANCHOR: f64 = 0.07;
const INITIAL_RADIUS: f64 = 10.0;

#[must_use]
pub fn graph(nodes: &[VizNode], edges: &[VizEdge]) -> LayoutResult {
    if nodes.is_empty() {
        LayoutResult {
            positions: HashMap::new(),
            width: PAD * 2.0,
            height: PAD * 2.0,
        }
    } else if is_forest(nodes, edges) {
        forest(nodes, edges)
    } else {
        force(nodes, edges)
    }
}

/// Acyclic and every node has at most one parent → a tree/forest we can lay out tidily.
fn is_forest(nodes: &[VizNode], edges: &[VizEdge]) -> bool {
    let mut in_deg: HashMap<&str, usize> = HashMap::new();
    for e in edges {
        *in_deg.entry(e.to.value()).or_default() += 1;
    }
    let single_parent = nodes
        .iter()
        .all(|n| in_deg.get(n.id.value()).copied().unwrap_or(0) <= 1);
    single_parent && !has_cycle(nodes, edges)
}

fn has_cycle(nodes: &[VizNode], edges: &[VizEdge]) -> bool {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in edges {
        adj.entry(e.from.value()).or_default().push(e.to.value());
    }
    // 0 unseen · 1 on-stack · 2 done; an explicit stack keeps deep graphs safe.
    let mut state: HashMap<&str, u8> = HashMap::new();
    for start in nodes.iter().map(|n| n.id.value()) {
        if state.get(start).copied().unwrap_or(0) != 0 {
            continue;
        }
        let mut stack: Vec<(&str, usize)> = vec![(start, 0)];
        state.insert(start, 1);
        while let Some((id, child_index)) = stack.pop() {
            let kids = adj.get(id).map(Vec::as_slice).unwrap_or_default();
            if let Some(next) = kids.get(child_index) {
                stack.push((id, child_index + 1));
                match state.get(next).copied().unwrap_or(0) {
                    1 => return true,
                    0 => {
                        state.insert(next, 1);
                        stack.push((next, 0));
                    }
                    _ => {}
                }
            } else {
                state.insert(id, 2);
            }
        }
    }
    false
}

// A forest: connect every root to a synthetic super-root, tidy-lay-out, drop the super-root.
fn forest(nodes: &[VizNode], edges: &[VizEdge]) -> LayoutResult {
    let has_parent: HashSet<&str> = edges.iter().map(|e| e.to.value()).collect();
    let roots: Vec<NodeId> = nodes
        .iter()
        .map(|n| n.id.clone())
        .filter(|id| !has_parent.contains(id.value()))
        .collect();
    if roots.len() <= 1 {
        return tree::tree(nodes, edges);
    }
    let super_root = NodeId::new("__syn_superroot");
    let mut aug_nodes = vec![VizNode {
        id: super_root.clone(),
        kind: "root".to_owned(),
        ..VizNode::default()
    }];
    aug_nodes.extend_from_slice(nodes);
    let mut aug_edges: Vec<VizEdge> = roots
        .iter()
        .map(|r| VizEdge {
            from: super_root.clone(),
            to: r.clone(),
            label: "child".to_owned(),
        })
        .collect();
    aug_edges.extend_from_slice(edges);
    let laid = tree::tree(&aug_nodes, &aug_edges);
    // Drop the synthetic root and lift everything up one row.
    let positions: HashMap<_, _> = laid
        .positions
        .into_iter()
        .filter(|(id, _)| *id != super_root)
        .map(|(id, p)| {
            (
                id,
                Point {
                    x: p.x,
                    y: p.y - TREE_ROW_H,
                },
            )
        })
        .collect();
    LayoutResult {
        positions,
        width: laid.width,
        height: (laid.height - TREE_ROW_H).max(PAD * 2.0),
    }
}

// The seeded force simulation. Loops run over indexed Vecs — the arithmetic ORDER is part of
// the determinism contract, so the phases stay one straight-line function rather than split
// helpers.
#[allow(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::too_many_lines
)]
fn force(nodes: &[VizNode], edges: &[VizEdge]) -> LayoutResult {
    let alpha_decay: f64 = 1.0 - 0.001_f64.powf(1.0 / 300.0); // ≈ 0.02276
    let initial_angle: f64 = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt()); // golden angle
    let collide_r: f64 = RING_R + 12.0;

    let mut rnd = Mulberry32(SEED);
    let ids: Vec<NodeId> = nodes.iter().map(|n| n.id.clone()).collect();
    let index: HashMap<&NodeId, usize> = ids.iter().enumerate().map(|(i, id)| (id, i)).collect();
    let n = ids.len();
    let mut x = vec![0.0_f64; n];
    let mut y = vec![0.0_f64; n];
    let mut vx = vec![0.0_f64; n];
    let mut vy = vec![0.0_f64; n];
    // d3 phyllotaxis init — deterministic, spreads nodes so forces don't start degenerate.
    for i in 0..n {
        let radius = INITIAL_RADIUS * (0.5 + i as f64).sqrt();
        let angle = i as f64 * initial_angle;
        x[i] = radius * angle.cos();
        y[i] = radius * angle.sin();
    }

    // Link endpoints as indices, plus degree for d3's strength/bias.
    let links: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|e| Some((*index.get(&e.from)?, *index.get(&e.to)?)))
        .collect();
    let mut degree = vec![0usize; n];
    for &(a, b) in &links {
        degree[a] += 1;
        degree[b] += 1;
    }

    let mut jiggle = move || (rnd.next() - 0.5) * 1e-6;
    let mut alpha = 1.0_f64;
    for _tick in 0..TICKS {
        alpha += (0.0 - alpha) * alpha_decay;
        // Many-body repulsion (naive O(n²)).
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let mut dx = x[j] - x[i];
                let mut dy = y[j] - y[i];
                let mut l = dx * dx + dy * dy;
                if l == 0.0 {
                    dx = jiggle();
                    dy = jiggle();
                    l = dx * dx + dy * dy;
                }
                if l < 1.0 {
                    l = l.sqrt(); // d3 distanceMin² clamp
                }
                let w = MANY_BODY * alpha / l;
                vx[i] += dx * w;
                vy[i] += dy * w;
            }
        }
        // Links pull toward LINK_DISTANCE (d3 strength/bias by degree).
        for &(a, b) in &links {
            let mut dx = (x[b] + vx[b]) - (x[a] + vx[a]);
            let mut dy = (y[b] + vy[b]) - (y[a] + vy[a]);
            let mut l = (dx * dx + dy * dy).sqrt();
            if l == 0.0 {
                dx = jiggle();
                dy = jiggle();
                l = (dx * dx + dy * dy).sqrt();
            }
            let strength = 1.0 / (degree[a].min(degree[b]).max(1)) as f64;
            let bias = degree[a] as f64 / (degree[a] + degree[b]).max(1) as f64;
            let ll = (l - LINK_DISTANCE) / l * alpha * strength;
            vx[b] -= dx * ll * bias;
            vy[b] -= dy * ll * bias;
            vx[a] += dx * ll * (1.0 - bias);
            vy[a] += dy * ll * (1.0 - bias);
        }
        // Collide: push apart nodes closer than 2·collide_r (equal split).
        for i in 0..n {
            for j in (i + 1)..n {
                let mut dx = (x[j] + vx[j]) - (x[i] + vx[i]);
                let mut dy = (y[j] + vy[j]) - (y[i] + vy[i]);
                let mut l = dx * dx + dy * dy;
                let r = collide_r * 2.0;
                if l < r * r {
                    if l == 0.0 {
                        dx = jiggle();
                        dy = jiggle();
                        l = dx * dx + dy * dy;
                    }
                    let d = l.sqrt();
                    let push = (r - d) / d * 0.5;
                    vx[i] -= dx * push;
                    vy[i] -= dy * push;
                    vx[j] += dx * push;
                    vy[j] += dy * push;
                }
            }
        }
        // x/y anchors toward the centre.
        for i in 0..n {
            vx[i] += (0.0 - x[i]) * ANCHOR * alpha;
            vy[i] += (0.0 - y[i]) * ANCHOR * alpha;
        }
        // Integrate (velocity-Verlet).
        for i in 0..n {
            vx[i] *= VELOCITY_DECAY;
            x[i] += vx[i];
            vy[i] *= VELOCITY_DECAY;
            y[i] += vy[i];
        }
    }

    // Shift the centred cloud into a positive, padded viewBox.
    let min_x = x.iter().copied().fold(f64::INFINITY, f64::min);
    let min_y = y.iter().copied().fold(f64::INFINITY, f64::min);
    let max_x = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let max_y = y.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let off = PAD + NODE_R;
    let positions: HashMap<_, _> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            (
                id.clone(),
                Point {
                    x: x[i] - min_x + off,
                    y: y[i] - min_y + off,
                },
            )
        })
        .collect();
    LayoutResult {
        positions,
        width: (max_x - min_x) + off * 2.0,
        height: (max_y - min_y) + off * 2.0,
    }
}

#[cfg(test)]
#[path = "graph_layout_tests.rs"]
mod tests;
