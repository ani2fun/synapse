//! The Tree family (oracle: `TreeLayouts.scala`): a recursive subtree-width walk — a leaf
//! takes the next column; an internal node centres over its children's columns; depth sets
//! the row. Children order by edge label (left < right < other), so a BST reads
//! left-to-right and a skewed chain cascades straight down.

use std::collections::{HashMap, HashSet};

use super::constants::{NODE_R, PAD, TREE_COL_W, TREE_ROW_H};
use super::{LayoutResult, Point};
use crate::engine::graph::{NodeId, VizEdge, VizNode};

fn label_order(label: &str) -> u8 {
    match label {
        "left" => 0,
        "right" => 1,
        _ => 2,
    }
}

struct Walk {
    children_of: HashMap<NodeId, Vec<NodeId>>,
    col: HashMap<NodeId, f64>,
    depth: HashMap<NodeId, i32>,
    visited: HashSet<NodeId>,
    next_leaf: f64,
}

impl Walk {
    // Iteration is over ordered Vecs throughout — never map order (the JVM↔JS lesson).
    fn walk(&mut self, id: &NodeId, d: i32) {
        if self.visited.contains(id) {
            return;
        }
        self.visited.insert(id.clone());
        self.depth.insert(id.clone(), d);
        let kids: Vec<NodeId> = self
            .children_of
            .get(id)
            .map(|ks| {
                ks.iter()
                    .filter(|k| !self.visited.contains(*k))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if kids.is_empty() {
            self.col.insert(id.clone(), self.next_leaf);
            self.next_leaf += 1.0;
        } else {
            for kid in &kids {
                self.walk(kid, d + 1);
            }
            let kid_cols: Vec<f64> = kids.iter().filter_map(|k| self.col.get(k).copied()).collect();
            let col = if kid_cols.is_empty() {
                let c = self.next_leaf;
                self.next_leaf += 1.0;
                c
            } else {
                #[allow(clippy::cast_precision_loss)]
                {
                    kid_cols.iter().sum::<f64>() / kid_cols.len() as f64
                }
            };
            self.col.insert(id.clone(), col);
        }
    }
}

/// Lay out a tree (binary or ordered n-ary) over the union of its nodes/edges.
#[must_use]
pub fn tree(nodes: &[VizNode], edges: &[VizEdge]) -> LayoutResult {
    let mut labelled: HashMap<NodeId, Vec<(u8, NodeId)>> = HashMap::new();
    for edge in edges {
        labelled
            .entry(edge.from.clone())
            .or_default()
            .push((label_order(&edge.label), edge.to.clone()));
    }
    let children_of: HashMap<NodeId, Vec<NodeId>> = labelled
        .into_iter()
        .map(|(id, mut kids)| {
            kids.sort_by_key(|(order, _)| *order);
            (id, kids.into_iter().map(|(_, k)| k).collect())
        })
        .collect();
    let has_parent: HashSet<&str> = edges.iter().map(|e| e.to.value()).collect();
    let roots: Vec<NodeId> = nodes
        .iter()
        .map(|n| n.id.clone())
        .filter(|id| !has_parent.contains(id.value()))
        .collect();
    // No clear root (a cycle, or every node has a parent) → start from the first node.
    let starts: Vec<NodeId> = if roots.is_empty() {
        nodes.first().map(|n| n.id.clone()).into_iter().collect()
    } else {
        roots
    };

    let mut walk = Walk {
        children_of,
        col: HashMap::new(),
        depth: HashMap::new(),
        visited: HashSet::new(),
        next_leaf: 0.0,
    };
    for start in &starts {
        walk.walk(start, 0);
    }
    // Any nodes not reached (disconnected) → fresh leaf columns at depth 0.
    for node in nodes {
        if !walk.visited.contains(&node.id) {
            walk.depth.insert(node.id.clone(), 0);
            walk.col.insert(node.id.clone(), walk.next_leaf);
            walk.next_leaf += 1.0;
        }
    }

    let positions: HashMap<_, _> = nodes
        .iter()
        .map(|n| {
            let x = PAD + NODE_R + walk.col.get(&n.id).copied().unwrap_or(0.0) * TREE_COL_W;
            let y = PAD + NODE_R + f64::from(walk.depth.get(&n.id).copied().unwrap_or(0)) * TREE_ROW_H;
            (n.id.clone(), Point { x, y })
        })
        .collect();
    let max_col = walk.col.values().copied().fold(0.0_f64, f64::max);
    let max_depth = walk.depth.values().copied().max().unwrap_or(0);
    LayoutResult {
        positions,
        width: PAD * 2.0 + NODE_R * 2.0 + max_col * TREE_COL_W,
        height: PAD * 2.0 + NODE_R * 2.0 + f64::from(max_depth) * TREE_ROW_H,
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
