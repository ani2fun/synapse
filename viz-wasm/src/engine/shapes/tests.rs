//! Inferring drawable structure from a raw step: bucket order in a hash table, direction and
//! cycle-safety in a chain, roots in a parent array, columns in a nested row. A step that already
//! carries edges passes through untouched — inference fills a gap, it does not overrule an author.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::engine::graph::VizField;

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
