//! The pure shape layer for the bespoke HTML families (oracle: `WidgetShapes.scala`,
//! step 33). DOM-free projections from a `VizStep`/`VizGraph` into the little models the
//! flow-layout renderers draw: hashmap buckets, the linked-list chain, the union-find
//! forest, the 2-D grid, and the heap slot-tree. Natively testable — the specs below are
//! the oracle's, case for case.

use std::collections::{HashMap, HashSet};

use crate::viz::engine::graph::{NodeId, VizCursor, VizEdge, VizGraph, VizNode, VizStep};

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
    // Numeric first (ascending), then text (lexicographic) — the oracle's tri-key sort.
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
/// trace is a list of lists; a flat row falls back to √n columns (mirrors `GridLayouts`).
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
// TESTS (oracle: WidgetShapesSpec, case for case)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::viz::engine::graph::VizField;

    fn cell(id: &str, label: &str, slot: i32) -> VizNode {
        VizNode {
            id: NodeId::new(id),
            label: label.to_owned(),
            kind: "cell".to_owned(),
            slot: Some(slot),
            ..VizNode::default()
        }
    }

    fn entry(id: &str, label: &str, key: &str) -> VizNode {
        VizNode {
            id: NodeId::new(id),
            label: label.to_owned(),
            kind: "entry".to_owned(),
            meta: vec![VizField {
                name: "key".to_owned(),
                value: key.to_owned(),
            }],
            ..VizNode::default()
        }
    }

    fn inst(id: &str, label: &str, meta: Vec<(&str, &str)>) -> VizNode {
        VizNode {
            id: NodeId::new(id),
            label: label.to_owned(),
            kind: "Entry".to_owned(),
            meta: meta
                .into_iter()
                .map(|(n, v)| VizField {
                    name: n.to_owned(),
                    value: v.to_owned(),
                })
                .collect(),
            ..VizNode::default()
        }
    }

    fn edge(from: &str, to: &str, label: &str) -> VizEdge {
        VizEdge {
            from: NodeId::new(from),
            to: NodeId::new(to),
            label: label.to_owned(),
        }
    }

    fn step(nodes: Vec<VizNode>, edges: Vec<VizEdge>, cursor: Vec<VizCursor>) -> VizStep {
        VizStep {
            nodes,
            edges,
            cursor,
            ..VizStep::default()
        }
    }

    fn cur(name: &str, target: &str) -> VizCursor {
        VizCursor {
            name: name.to_owned(),
            target: NodeId::new(target),
            color: String::new(),
        }
    }

    #[test]
    fn toy_hash_table_rebuilds_entry_cells_instances_with_numeric_bucket_order() {
        let s = step(
            vec![
                entry("d#3", "·", "3"),
                entry("d#1", "·", "1"),
                cell("l1#0", "·", 0),
                cell("l1#1", "·", 1),
                cell("l3#0", "·", 0),
                inst("e1", "apple", vec![("key", "1")]),
                inst("e2", "grape", vec![("key", "2")]),
                inst("e3", "fig", vec![("key", "3")]),
            ],
            vec![
                edge("d#1", "l1#0", ""),
                edge("d#1", "l1#1", ""),
                edge("d#3", "l3#0", ""),
                edge("l1#0", "e1", ""),
                edge("l1#1", "e2", ""),
                edge("l3#0", "e3", ""),
            ],
            vec![],
        );
        let bs = buckets(&s);
        assert_eq!(
            bs.iter().map(|b| b.index.as_str()).collect::<Vec<_>>(),
            ["1", "3"]
        );
        assert_eq!(
            bs[0]
                .entries
                .iter()
                .map(|e| (e.key.clone(), e.value.as_str()))
                .collect::<Vec<_>>(),
            [(Some("1".to_owned()), "apple"), (Some("2".to_owned()), "grape")]
        );
        assert_eq!(
            bs[1]
                .entries
                .iter()
                .map(|e| (e.key.clone(), e.value.as_str()))
                .collect::<Vec<_>>(),
            [(Some("3".to_owned()), "fig")]
        );
    }

    #[test]
    fn a_plain_scalar_dict_is_one_pill_per_key_text_keys_after_numeric() {
        let s = step(
            vec![entry("d#b", "2", "b"), entry("d#10", "1", "10")],
            vec![],
            vec![],
        );
        let bs = buckets(&s);
        assert_eq!(
            bs.iter().map(|b| b.index.as_str()).collect::<Vec<_>>(),
            ["10", "b"]
        );
        assert_eq!(
            bs[0].entries,
            vec![BucketEntry {
                id: NodeId::new("d#10"),
                key: None,
                value: "1".to_owned(),
            }]
        );
        assert_eq!(
            bs[1].entries.iter().map(|e| e.value.as_str()).collect::<Vec<_>>(),
            ["2"]
        );
    }

    #[test]
    fn a_ref_bucket_with_no_reachable_chain_reads_empty() {
        let s = step(vec![entry("d#0", "·", "0")], vec![], vec![]);
        assert!(buckets(&s)[0].entries.is_empty());
    }

    #[test]
    fn chain_orders_from_the_head_cursor_and_detects_singly() {
        let s = step(
            vec![
                inst("b", "20", vec![]),
                inst("a", "10", vec![]),
                inst("c", "30", vec![]),
            ],
            vec![edge("a", "b", "next"), edge("b", "c", "next")],
            vec![cur("head", "a")],
        );
        let info = chain(&s);
        assert_eq!(
            info.nodes.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(),
            ["10", "20", "30"]
        );
        assert!(!info.is_doubly);
    }

    #[test]
    fn headless_starts_at_no_incoming_next_stragglers_append_prev_means_doubly() {
        let s = step(
            vec![
                inst("b", "20", vec![]),
                inst("a", "10", vec![]),
                inst("x", "99", vec![]),
            ],
            vec![edge("a", "b", "next"), edge("b", "a", "prev")],
            vec![],
        );
        let info = chain(&s);
        assert_eq!(
            info.nodes.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(),
            ["10", "20", "99"]
        );
        assert!(info.is_doubly);
    }

    #[test]
    fn a_cycle_terminates_visited_guarded() {
        let s = step(
            vec![inst("a", "1", vec![]), inst("b", "2", vec![])],
            vec![edge("a", "b", "next"), edge("b", "a", "next")],
            vec![cur("head", "a")],
        );
        let info = chain(&s);
        assert_eq!(
            info.nodes.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(),
            ["1", "2"]
        );
    }

    #[test]
    fn parent_array_self_loop_cells_are_roots_others_carry_their_parent() {
        let s = step(
            vec![cell("p#0", "0", 0), cell("p#1", "0", 1), cell("p#2", "2", 2)],
            vec![],
            vec![],
        );
        let es = forest(&s);
        assert_eq!(
            es.iter().map(|e| e.is_root).collect::<Vec<_>>(),
            [true, false, true]
        );
        assert_eq!(es[1].parent, Some(0));
    }

    #[test]
    fn forest_graph_relabels_by_index_draws_parent_edges_badges_roots() {
        let g = VizGraph {
            steps: vec![step(
                vec![cell("p#0", "0", 0), cell("p#1", "0", 1), cell("p#2", "2", 2)],
                vec![],
                vec![],
            )],
            ..VizGraph::default()
        };
        let fg = forest_graph(&g);
        let s = &fg.steps[0];
        assert_eq!(
            s.nodes.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(),
            ["0", "1", "2"]
        );
        assert_eq!(s.edges, vec![edge("p#0", "p#1", "")]);
        assert_eq!(s.cursor.iter().filter(|c| c.name == "root").count(), 2);
    }

    #[test]
    fn nested_rows_follow_the_outer_ref_cells_columns_by_slot() {
        let s = step(
            vec![
                cell("g#0", "·", 0),
                cell("g#1", "·", 1),
                cell("r0#0", "1", 0),
                cell("r0#1", "2", 1),
                cell("r1#0", "3", 0),
                cell("r1#1", "4", 1),
            ],
            vec![
                edge("g#0", "r0#0", ""),
                edge("g#0", "r0#1", ""),
                edge("g#1", "r1#0", ""),
                edge("g#1", "r1#1", ""),
            ],
            vec![],
        );
        let rows = grid_cells(&s);
        let labels: Vec<Vec<&str>> = rows
            .iter()
            .map(|r| r.iter().flatten().map(|n| n.label.as_str()).collect())
            .collect();
        assert_eq!(labels, [["1", "2"], ["3", "4"]]);
    }

    #[test]
    fn a_flat_row_falls_back_to_sqrt_n_columns() {
        let s = step(
            vec![
                cell("c0", "1", 0),
                cell("c1", "2", 1),
                cell("c2", "3", 2),
                cell("c3", "4", 3),
            ],
            vec![],
            vec![],
        );
        let rows = grid_cells(&s);
        assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2]);
    }

    #[test]
    fn heap_tree_synthesizes_the_slot_tree() {
        let g = VizGraph {
            steps: vec![step(
                vec![cell("h#0", "1", 0), cell("h#1", "3", 1), cell("h#2", "2", 2)],
                vec![],
                vec![],
            )],
            ..VizGraph::default()
        };
        let edges: HashSet<VizEdge> = heap_tree(&g).steps[0].edges.iter().cloned().collect();
        assert_eq!(
            edges,
            HashSet::from([edge("h#0", "h#1", "left"), edge("h#0", "h#2", "right")])
        );
    }

    #[test]
    fn steps_that_already_carry_edges_pass_through_untouched() {
        let g = VizGraph {
            steps: vec![step(
                vec![inst("a", "1", vec![]), inst("b", "2", vec![])],
                vec![edge("a", "b", "left")],
                vec![],
            )],
            ..VizGraph::default()
        };
        assert_eq!(heap_tree(&g).steps[0].edges, vec![edge("a", "b", "left")]);
    }
}
