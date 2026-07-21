//! Oracle: `ArrayLayoutSpec` (incl. `Layout.union`) + `StackLayoutSpec`.

#![allow(clippy::unwrap_used, clippy::float_cmp)] // exact constants are the point

use super::super::constants::{CARET_ROW_H, CELL_DX, CELL_GAP, CELL_H, INDEX_ROW_H, PAD};
use super::super::{Point, union};
use super::*;
use crate::engine::graph::{NodeId, VizGraph, VizStep};

fn cell(id: &str, slot: i32) -> VizNode {
    VizNode {
        id: NodeId::new(id),
        label: id.to_owned(),
        kind: "cell".to_owned(),
        slot: Some(slot),
        ..VizNode::default()
    }
}

fn x_of(r: &LayoutResult, id: &str) -> f64 {
    r.positions[&NodeId::new(id)].x
}

fn y_of(r: &LayoutResult, id: &str) -> f64 {
    r.positions[&NodeId::new(id)].y
}

// ── array ─────────────────────────────────────────────────────────────────────

#[test]
fn a_single_cell_sits_at_pad_caret_row() {
    let r = array(&[cell("a", 0)], &[]);
    assert_eq!(
        r.positions[&NodeId::new("a")],
        Point {
            x: PAD,
            y: CARET_ROW_H
        }
    );
}

#[test]
fn columns_stride_by_cell_dx_per_slot_regardless_of_input_order() {
    let r = array(&[cell("c", 2), cell("a", 0), cell("b", 1)], &[]);
    assert_eq!(x_of(&r, "a"), PAD);
    assert_eq!(x_of(&r, "b"), PAD + CELL_DX);
    assert_eq!(x_of(&r, "c"), PAD + 2.0 * CELL_DX);
}

#[test]
fn a_slot_gap_leaves_the_column_empty() {
    let r = array(&[cell("a", 0), cell("c", 2)], &[]);
    assert_eq!(x_of(&r, "c"), PAD + 2.0 * CELL_DX);
}

#[test]
fn all_cells_share_the_caret_row_baseline() {
    let r = array(&[cell("a", 0), cell("b", 1)], &[]);
    assert!(r.positions.values().all(|p| p.y == CARET_ROW_H));
}

#[test]
fn a_slot_less_node_falls_to_column_zero() {
    let node = VizNode {
        id: NodeId::new("x"),
        label: "x".to_owned(),
        kind: "cell".to_owned(),
        ..VizNode::default()
    };
    let r = array(&[node], &[]);
    assert_eq!(x_of(&r, "x"), PAD);
}

#[test]
fn width_scales_with_the_max_slot() {
    let one = array(&[cell("a", 0)], &[]);
    let two = array(&[cell("a", 0), cell("b", 1)], &[]);
    assert_eq!(two.width, one.width + CELL_DX);
}

#[test]
fn height_reserves_caret_cell_and_index_rows() {
    let r = array(&[cell("a", 0)], &[]);
    assert_eq!(r.height, CARET_ROW_H + CELL_H + INDEX_ROW_H);
}

#[test]
fn no_nodes_is_an_empty_minimal_layout() {
    let r = array(&[], &[]);
    assert!(r.positions.is_empty());
    assert!(r.width > 0.0 && r.height > 0.0);
}

// ── union ─────────────────────────────────────────────────────────────────────

fn edge(f: &str, t: &str, l: &str) -> VizEdge {
    VizEdge {
        from: NodeId::new(f),
        to: NodeId::new(t),
        label: l.to_owned(),
    }
}

#[test]
fn union_dedups_nodes_by_id_across_steps_and_keeps_edges_once() {
    let g = VizGraph {
        steps: vec![
            VizStep {
                nodes: vec![cell("a", 0), cell("b", 1)],
                edges: vec![edge("a", "b", "next")],
                ..VizStep::default()
            },
            VizStep {
                nodes: vec![cell("a", 0), cell("c", 2)],
                edges: vec![edge("a", "b", "next")],
                ..VizStep::default()
            },
        ],
        ..VizGraph::default()
    };
    let (nodes, edges) = union(&g);
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.value()).collect();
    assert_eq!(ids, vec!["a", "b", "c"]);
    assert_eq!(edges.len(), 1);
}

#[test]
fn union_lays_out_cells_that_appear_only_in_later_steps() {
    let g = VizGraph {
        steps: vec![
            VizStep {
                nodes: vec![cell("a", 0)],
                ..VizStep::default()
            },
            VizStep {
                nodes: vec![cell("a", 0), cell("b", 1)],
                ..VizStep::default()
            },
        ],
        ..VizGraph::default()
    };
    let (nodes, _) = union(&g);
    let r = array(&nodes, &[]);
    assert!(r.positions.contains_key(&NodeId::new("b")));
}

// ── stack ─────────────────────────────────────────────────────────────────────

#[test]
fn slot_zero_sits_at_the_bottom() {
    let r = stack(&[cell("a", 0), cell("b", 1), cell("c", 2)], &[]);
    assert!(y_of(&r, "a") > y_of(&r, "b") && y_of(&r, "b") > y_of(&r, "c"));
}

#[test]
fn a_higher_slot_stacks_upward_by_one_stride() {
    let r = stack(&[cell("a", 0), cell("b", 1)], &[]);
    assert_eq!(y_of(&r, "a") - y_of(&r, "b"), CELL_H + CELL_GAP);
}

#[test]
fn all_cells_share_the_same_column() {
    let r = stack(&[cell("a", 0), cell("b", 1), cell("c", 2)], &[]);
    let xs: std::collections::HashSet<u64> = r.positions.values().map(|p| p.x.to_bits()).collect();
    assert_eq!(xs.len(), 1);
}

#[test]
fn a_row_is_reserved_on_top_for_the_top_marker() {
    let r = stack(&[cell("top", 0)], &[]);
    assert!(y_of(&r, "top") >= PAD + CARET_ROW_H);
}

#[test]
fn a_single_element_lays_out() {
    let r = stack(&[cell("only", 0)], &[]);
    assert!(r.positions.contains_key(&NodeId::new("only")));
    assert!(r.height > 0.0 && r.width > 0.0);
}

#[test]
fn height_grows_with_the_stack_depth() {
    let one = stack(&[cell("a", 0)], &[]);
    let two = stack(&[cell("a", 0), cell("b", 1)], &[]);
    assert!(two.height > one.height);
}

#[test]
fn no_nodes_is_a_minimal_stack_layout() {
    let r = stack(&[], &[]);
    assert!(r.positions.is_empty());
    assert!(r.width > 0.0 && r.height > 0.0);
}
