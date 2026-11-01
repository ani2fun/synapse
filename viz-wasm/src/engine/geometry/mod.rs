//! Pure geometry: `(nodes, edges) → positions`.
//! A layout is a pure function of the UNION of every step's nodes/edges (the stability
//! invariant): a node's position NEVER shifts between steps — per-step rendering
//! only toggles presence/classes. `constants` is the one place layout numbers live, so a
//! change lands once instead of needing to be copied to every family.

pub mod chain;
pub mod constants;
pub mod graph_layout;
pub mod grid;
pub mod linear;
pub mod placed;
pub mod tree;

use std::collections::{HashMap, HashSet};

use crate::engine::graph::{NodeId, VizEdge, VizGraph, VizNode};

/// A laid-out point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Positions for every node id, plus the figure's intrinsic size (→ the SVG viewBox).
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutResult {
    pub positions: HashMap<NodeId, Point>,
    pub width: f64,
    pub height: f64,
}

/// The union of every step's nodes (deduped by id, FIRST occurrence wins) and edges — what a
/// layout is computed over, once.
#[must_use]
pub fn union(graph: &VizGraph) -> (Vec<VizNode>, Vec<VizEdge>) {
    let mut seen_nodes = HashSet::new();
    let mut nodes = Vec::new();
    let mut seen_edges = HashSet::new();
    let mut edges = Vec::new();
    for step in &graph.steps {
        for node in &step.nodes {
            if seen_nodes.insert(node.id.value().to_owned()) {
                nodes.push(node.clone());
            }
        }
        for edge in &step.edges {
            let key = (
                edge.from.value().to_owned(),
                edge.to.value().to_owned(),
                edge.label.clone(),
            );
            if seen_edges.insert(key) {
                edges.push(edge.clone());
            }
        }
    }
    (nodes, edges)
}

/// The point at `t` (0..=1) along the cubic Bézier with control points `c`.
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
