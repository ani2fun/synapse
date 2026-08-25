//! The pure shape layer for the bespoke HTML families. DOM-free projections from a
//! `VizStep`/`VizGraph` into the little models the flow-layout renderers draw: hashmap
//! buckets, the linked-list chain, the union-find forest, the 2-D grid, and the heap
//! slot-tree. Natively testable.

use std::collections::{HashMap, HashSet};

use crate::engine::graph::{NodeId, VizCursor, VizEdge, VizGraph, VizNode, VizStep};

/// A ref-valued node's label (mirrors `AdaptVocab.RefLabel`).
const REF_LABEL: &str = "·";

const NEXT_LABELS: [&str; 2] = ["next", "nxt"];
const PREV_LABELS: [&str; 2] = ["prev", "previous"];

// ─────────────────────────────────────────────────────────────────────────────
// HASHMAP BUCKETS
// ─────────────────────────────────────────────────────────────────────────────

/// One `key: value` pill in a bucket's chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketEntry {
    pub id: NodeId,
    pub key: Option<String>,
    pub value: String,
}

/// One bucket row: the index chip + its chain of pills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bucket {
    pub entry_id: NodeId,
    pub index: String,
    pub entries: Vec<BucketEntry>,
}

fn by_id(step: &VizStep) -> HashMap<&str, &VizNode> {
    step.nodes.iter().map(|n| (n.id.value(), n)).collect()
}

fn by_from(step: &VizStep) -> HashMap<&str, Vec<&VizEdge>> {
    let mut m: HashMap<&str, Vec<&VizEdge>> = HashMap::new();
    for e in &step.edges {
        m.entry(e.from.value()).or_default().push(e);
    }
    m
}

fn meta_key(n: &VizNode) -> Option<String> {
    n.meta.iter().find(|f| f.name == "key").map(|f| f.value.clone())
}

fn pill(n: &VizNode) -> BucketEntry {
    BucketEntry {
        id: n.id.clone(),
        key: meta_key(n),
        value: n.label.clone(),
    }
}

/// A dict step → its buckets: each `kind == "entry"` node is a bucket; a ref entry (`·`)
/// walks entry → cells → instances; a scalar entry is one pill whose value is its own label.
/// Numeric bucket indices sort first in numeric order, text ones after, lexicographically.
#[must_use]
pub fn buckets(step: &VizStep) -> Vec<Bucket> {
    let ids = by_id(step);
    let froms = by_from(step);
    let chain_of = |entry: &VizNode| -> Vec<BucketEntry> {
        let targets: Vec<&VizNode> = froms
            .get(entry.id.value())
            .map(|es| es.iter().filter_map(|e| ids.get(e.to.value()).copied()).collect())
            .unwrap_or_default();
        let mut cells: Vec<&VizNode> = targets.iter().filter(|n| n.kind == "cell").copied().collect();
        cells.sort_by_key(|n| n.slot.unwrap_or(i32::MAX));
        let direct: Vec<&VizNode> = targets.iter().filter(|n| n.kind != "cell").copied().collect();
        let via_cells = cells.iter().map(|c| {
            froms
                .get(c.id.value())
                .and_then(|es| es.first())
                .and_then(|e| ids.get(e.to.value()))
                .map_or_else(
                    // A list cell with no out-edge IS the value.
                    || BucketEntry {
                        id: c.id.clone(),
                        key: None,
                        value: c.label.clone(),
                    },
                    |inst| pill(inst),
                )
        });
        via_cells.chain(direct.into_iter().map(pill)).collect()
    };
    let mut out: Vec<Bucket> = step
        .nodes
        .iter()
        .filter(|n| n.kind == "entry")
        .map(|entry| {
            let entries = if entry.label == REF_LABEL {
                chain_of(entry)
            } else {
                vec![BucketEntry {
                    id: entry.id.clone(),
                    key: None,
                    value: entry.label.clone(),
                }]
            };
            Bucket {
                entry_id: entry.id.clone(),
                index: meta_key(entry).unwrap_or_else(|| "?".to_owned()),
                entries,
            }
        })
        .collect();
    // Numeric first (ascending), then text (lexicographic) — a tri-key sort.
    out.sort_by(|a, b| {
        let na = a.index.parse::<f64>().ok();
        let nb = b.index.parse::<f64>().ok();
        na.is_none()
            .cmp(&nb.is_none())
            .then_with(|| na.unwrap_or(0.0).total_cmp(&nb.unwrap_or(0.0)))
            .then_with(|| a.index.cmp(&b.index))
    });
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// LINKED-LIST CHAIN
// ─────────────────────────────────────────────────────────────────────────────

/// The list's nodes in walk order + whether any `prev` edge makes it doubly linked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainInfo {
    pub nodes: Vec<VizNode>,
    pub is_doubly: bool,
}

/// Order the list: start at the head cursor (else the node with no incoming `next`), walk
/// `next` edges cycle-guarded, then append unreached stragglers in wire order.
#[must_use]
pub fn chain(step: &VizStep) -> ChainInfo {
    let ids = by_id(step);
    let next_of: HashMap<&str, &str> = step
        .edges
        .iter()
        .filter(|e| NEXT_LABELS.contains(&e.label.as_str()))
        .map(|e| (e.from.value(), e.to.value()))
        .collect();
    let has_incoming_next: HashSet<&str> = step
        .edges
        .iter()
        .filter(|e| NEXT_LABELS.contains(&e.label.as_str()))
        .map(|e| e.to.value())
        .collect();
    let start: Option<&str> = step
        .cursor
        .iter()
        .find(|c| matches!(c.name.as_str(), "head" | "h" | "first"))
        .map(|c| c.target.value())
        .filter(|t| ids.contains_key(t))
        .or_else(|| {
            step.nodes
                .iter()
                .find(|n| !has_incoming_next.contains(n.id.value()))
                .map(|n| n.id.value())
        });
    let mut seen: HashSet<&str> = HashSet::new();
    let mut ordered: Vec<VizNode> = Vec::new();
    let mut cur = start;
    while let Some(id) = cur {
        if seen.contains(id) {
            break;
        }
        let Some(node) = ids.get(id) else { break };
        seen.insert(id);
        ordered.push((*node).clone());
        cur = next_of.get(id).copied();
    }
    let stragglers = step
        .nodes
        .iter()
        .filter(|n| !seen.contains(n.id.value()))
        .cloned();
    ordered.extend(stragglers);
    ChainInfo {
        nodes: ordered,
        is_doubly: step.edges.iter().any(|e| PREV_LABELS.contains(&e.label.as_str())),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UNION-FIND FOREST
// ─────────────────────────────────────────────────────────────────────────────

/// One parent-array element: its slot, the parent slot its label encodes, rootness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UfElem {
    pub id: NodeId,
    pub slot: i32,
    pub parent: Option<i32>,
    pub is_root: bool,
}

/// The parent array read structurally: a cell is a root iff its label is unparseable OR
/// points at its own slot.
#[must_use]
pub fn forest(step: &VizStep) -> Vec<UfElem> {
    let mut cells: Vec<&VizNode> = step.nodes.iter().filter(|n| n.kind == "cell").collect();
    cells.sort_by_key(|n| n.slot.unwrap_or(i32::MAX));
    cells
        .iter()
        .enumerate()
        .map(|(i, n)| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let slot = n.slot.unwrap_or(i as i32);
            let parent = n.label.trim().parse::<i32>().ok();
            UfElem {
                id: n.id.clone(),
                slot,
                parent,
                is_root: parent.is_none_or(|p| p == slot),
            }
        })
        .collect()
}

/// The drawable forest: nodes relabelled by ELEMENT INDEX (the slot), parent→child edges,
/// and a synthetic `root` cursor badging every root. Diff cues carry over untouched (same
/// node ids as the backing array).
#[must_use]
pub fn forest_graph(graph: &VizGraph) -> VizGraph {
    let steps = graph
        .steps
        .iter()
        .map(|step| {
            let elems = forest(step);
            let by_slot: HashMap<i32, &UfElem> = elems.iter().map(|e| (e.slot, e)).collect();
            let ids = by_id(step);
            let nodes: Vec<VizNode> = elems
                .iter()
                .filter_map(|e| ids.get(e.id.value()).copied())
                .zip(&elems)
                .map(|(n, e)| VizNode {
                    label: e.slot.to_string(),
                    kind: "ufnode".to_owned(),
                    ..n.clone()
                })
                .collect();
            let edges: Vec<VizEdge> = elems
                .iter()
                .filter(|e| !e.is_root)
                .filter_map(|e| {
                    e.parent.and_then(|p| by_slot.get(&p)).map(|parent| VizEdge {
                        from: parent.id.clone(),
                        to: e.id.clone(),
                        label: String::new(),
                    })
                })
                .collect();
            let roots = elems.iter().filter(|e| e.is_root).map(|e| VizCursor {
                name: "root".to_owned(),
                target: e.id.clone(),
                color: "#3a5a8c".to_owned(),
            });
            let mut cursor = step.cursor.clone();
            cursor.extend(roots);
            VizStep {
                nodes,
                edges,
                cursor,
                ..step.clone()
            }
        })
        .collect();
    VizGraph {
        steps,
        ..graph.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2-D GRID
// ─────────────────────────────────────────────────────────────────────────────

/// Row-major cells; holes stay `None`. Nested rows follow the outer ref-cells when the
/// trace is a list of lists; a flat row falls back to √n columns.
#[must_use]
pub fn grid_cells(step: &VizStep) -> Vec<Vec<Option<VizNode>>> {
    let ids = by_id(step);
    let froms = by_from(step);
    let row_of = |cells: Vec<&VizNode>| -> Vec<Option<VizNode>> {
        let by_slot: HashMap<i32, &VizNode> = cells.iter().filter_map(|c| c.slot.map(|s| (s, *c))).collect();
        let max_slot = by_slot.keys().copied().max().unwrap_or(-1);
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let width = (max_slot + 1).max(cells.len() as i32);
        (0..width)
            .map(|i| by_slot.get(&i).map(|n| (*n).clone()))
            .collect()
    };
    let mut outer: Vec<&VizNode> = step
        .nodes
        .iter()
        .filter(|n| n.kind == "cell" && n.label == REF_LABEL && froms.contains_key(n.id.value()))
        .collect();
    outer.sort_by_key(|n| n.slot.unwrap_or(i32::MAX));
    if !outer.is_empty() {
        return outer
            .iter()
            .map(|o| {
                let mut cells: Vec<&VizNode> = froms
                    .get(o.id.value())
                    .map(|es| {
                        es.iter()
                            .filter_map(|e| ids.get(e.to.value()).copied())
                            .filter(|n| n.kind == "cell")
                            .collect()
                    })
                    .unwrap_or_default();
                cells.sort_by_key(|n| n.slot.unwrap_or(i32::MAX));
                row_of(cells)
            })
            .collect();
    }
    let mut cells: Vec<&VizNode> = step.nodes.iter().filter(|n| n.kind == "cell").collect();
    cells.sort_by_key(|n| n.slot.unwrap_or(i32::MAX));
    if cells.is_empty() {
        return Vec::new();
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let cols = ((cells.len() as f64).sqrt().round() as usize).max(1);
    cells
        .chunks(cols)
        .map(|chunk| {
            let mut row: Vec<Option<VizNode>> = chunk.iter().map(|n| Some((*n).clone())).collect();
            row.resize(cols, None);
            row
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// HEAP SLOT-TREE
// ─────────────────────────────────────────────────────────────────────────────

/// The heap's tree view: a bare-array step synthesizes `i → 2i+1 (left) · 2i+2 (right)`
/// edges; a step that already carries edges (an object heap) passes through untouched.
#[must_use]
pub fn heap_tree(graph: &VizGraph) -> VizGraph {
    let steps = graph
        .steps
        .iter()
        .map(|step| {
            if !step.edges.is_empty() {
                return step.clone();
            }
            let mut cells: Vec<&VizNode> = step.nodes.iter().filter(|n| n.kind == "cell").collect();
            cells.sort_by_key(|n| n.slot.unwrap_or(i32::MAX));
            let by_slot: HashMap<i32, &VizNode> =
                cells.iter().filter_map(|c| c.slot.map(|s| (s, *c))).collect();
            let edges: Vec<VizEdge> = cells
                .iter()
                .filter_map(|n| n.slot.map(|s| (s, *n)))
                .flat_map(|(slot, node)| {
                    [(2 * slot + 1, "left"), (2 * slot + 2, "right")]
                        .into_iter()
                        .filter_map(|(child, side)| {
                            by_slot.get(&child).map(|c| VizEdge {
                                from: node.id.clone(),
                                to: c.id.clone(),
                                label: side.to_owned(),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            VizStep {
                edges,
                ..step.clone()
            }
        })
        .collect();
    VizGraph {
        steps,
        ..graph.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTS
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
