//! Pure geometry: `(nodes, edges) → positions` (oracle: `viz/geometry`, ADR-S026/S028).
//! A layout is a pure function of the UNION of every step's nodes/edges (the stability
//! invariant, ADR-0018): a node's position NEVER shifts between steps — per-step rendering
//! only toggles presence/classes. `constants` is the one place layout numbers live (Cortex
//! copy-pasted `NODE_R=22` across ~11 files; here a change lands once).

pub mod chain;
pub mod constants;
pub mod graph_layout;
pub mod grid;
pub mod linear;
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
