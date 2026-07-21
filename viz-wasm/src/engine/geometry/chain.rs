//! The Chain family (oracle: `ChainLayouts.scala`): nodes laid out left-to-right in
//! `next`-chain order from the head (the node with no incoming `next`). `prev` edges are
//! IGNORED for placement (the renderer draws them as back-arrows). A cycle stops the walk at
//! the first repeat; nodes off the chain are appended. `graph_fallback` flags a cycle/merge
//! so pathological lists can route to the graph canvas.

use std::collections::{HashMap, HashSet};

use super::constants::{CHAIN_DX, NODE_R, PAD};
use super::{LayoutResult, Point};
use crate::engine::graph::{NodeId, VizEdge, VizNode};

/// A chain layout plus a flag: a cycle or a merge (two nexts into one node) means "prefer the
/// graph canvas".
#[derive(Debug, Clone, PartialEq)]
pub struct ChainLayout {
    pub result: LayoutResult,
    pub graph_fallback: bool,
}

#[must_use]
pub fn chain(nodes: &[VizNode], edges: &[VizEdge]) -> ChainLayout {
    let next_edges: Vec<&VizEdge> = edges.iter().filter(|e| e.label == "next").collect();
    let next_of: HashMap<&NodeId, &NodeId> = next_edges.iter().map(|e| (&e.from, &e.to)).collect();
    let next_targets: Vec<&NodeId> = next_edges.iter().map(|e| &e.to).collect();
    let distinct_targets: HashSet<&NodeId> = next_targets.iter().copied().collect();
    // Two nodes point `next` at the same node.
    let merge = distinct_targets.len() != next_targets.len();
    let head = nodes
        .iter()
        .map(|n| &n.id)
        .find(|id| !distinct_targets.contains(*id))
        .or_else(|| nodes.first().map(|n| &n.id));

    let mut order: Vec<NodeId> = Vec::new();
    let mut seen: HashSet<NodeId> = HashSet::new();
    let mut cur = head;
    let mut cycle = false;
    while let Some(id) = cur {
        if seen.contains(id) {
            cycle = true;
            cur = None;
        } else {
            order.push(id.clone());
            seen.insert(id.clone());
            cur = next_of.get(id).copied();
        }
    }

    let mut full = order;
    full.extend(nodes.iter().map(|n| n.id.clone()).filter(|id| !seen.contains(id)));
    #[allow(clippy::cast_precision_loss)]
    let positions: HashMap<_, _> = full
        .iter()
        .enumerate()
        .map(|(i, id)| {
            (
                id.clone(),
                Point {
                    x: PAD + NODE_R + i as f64 * CHAIN_DX,
                    y: PAD + NODE_R,
                },
            )
        })
        .collect();
    #[allow(clippy::cast_precision_loss)]
    let width = PAD * 2.0 + NODE_R * 2.0 + (full.len().saturating_sub(1)) as f64 * CHAIN_DX + CHAIN_DX;
    ChainLayout {
        result: LayoutResult {
            positions,
            width,
            height: PAD * 2.0 + NODE_R * 2.0,
        },
        graph_fallback: cycle || merge,
    }
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
