//! The subtree-width walk over a complete BST.

#![allow(clippy::unwrap_used, clippy::float_cmp)] // exact constants are the point

use super::super::Point;
use super::super::constants::{NODE_R, PAD, TREE_COL_W, TREE_ROW_H};
use super::*;

fn node(id: &str) -> VizNode {
    VizNode {
        id: NodeId::new(id),
        label: id.to_owned(),
        kind: "node".to_owned(),
        ..VizNode::default()
    }
}

fn edge(f: &str, t: &str, l: &str) -> VizEdge {
    VizEdge {
        from: NodeId::new(f),
        to: NodeId::new(t),
        label: l.to_owned(),
    }
}

// A complete BST: 4 over (2 over 1,3) and (6 over 5,7)
fn bst_nodes() -> Vec<VizNode> {
    ["n4", "n2", "n6", "n1", "n3", "n5", "n7"].map(node).to_vec()
}

fn bst_edges() -> Vec<VizEdge> {
    vec![
        edge("n4", "n2", "left"),
        edge("n4", "n6", "right"),
        edge("n2", "n1", "left"),
        edge("n2", "n3", "right"),
        edge("n6", "n5", "left"),
        edge("n6", "n7", "right"),
    ]
}

fn x_of(r: &LayoutResult, id: &str) -> f64 {
    r.positions[&NodeId::new(id)].x
}

fn y_of(r: &LayoutResult, id: &str) -> f64 {
    r.positions[&NodeId::new(id)].y
}

#[test]
fn the_root_sits_at_the_top() {
    let r = tree(&bst_nodes(), &bst_edges());
    assert!(
        bst_nodes()
            .iter()
            .all(|n| y_of(&r, "n4") <= y_of(&r, n.id.value()))
    );
}

#[test]
fn depth_increases_y_by_one_row_per_level() {
    let r = tree(&bst_nodes(), &bst_edges());
    assert_eq!(y_of(&r, "n2"), y_of(&r, "n4") + TREE_ROW_H);
    assert_eq!(y_of(&r, "n1"), y_of(&r, "n4") + 2.0 * TREE_ROW_H);
}

#[test]
fn a_left_child_is_left_of_its_right_sibling() {
    let r = tree(&bst_nodes(), &bst_edges());
    assert!(x_of(&r, "n1") < x_of(&r, "n3"));
    assert!(x_of(&r, "n2") < x_of(&r, "n6"));
    assert!(x_of(&r, "n5") < x_of(&r, "n7"));
}

#[test]
fn an_internal_node_is_centred_between_its_children() {
    let r = tree(&bst_nodes(), &bst_edges());
    assert!(x_of(&r, "n2") < x_of(&r, "n4") && x_of(&r, "n4") < x_of(&r, "n6"));
    assert!((x_of(&r, "n4") - f64::midpoint(x_of(&r, "n2"), x_of(&r, "n6"))).abs() < 0.001);
}

#[test]
fn in_order_columns_are_strictly_increasing() {
    let r = tree(&bst_nodes(), &bst_edges());
    let xs: Vec<f64> = ["n1", "n2", "n3", "n4", "n5", "n6", "n7"]
        .iter()
        .map(|id| x_of(&r, id))
        .collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn a_single_node_lays_out_at_the_padded_origin() {
    let r = tree(&[node("only")], &[]);
    assert_eq!(
        r.positions[&NodeId::new("only")],
        Point {
            x: PAD + NODE_R,
            y: PAD + NODE_R
        }
    );
}

#[test]
fn a_left_only_chain_cascades_straight_down() {
    let ns = ["a", "b", "c"].map(node).to_vec();
    let es = vec![edge("a", "b", "left"), edge("b", "c", "left")];
    let r = tree(&ns, &es);
    assert!(x_of(&r, "a") == x_of(&r, "b") && x_of(&r, "b") == x_of(&r, "c"));
    assert!(y_of(&r, "a") < y_of(&r, "b") && y_of(&r, "b") < y_of(&r, "c"));
}

#[test]
fn a_disconnected_node_is_still_placed() {
    let mut ns = bst_nodes();
    ns.push(node("loner"));
    let r = tree(&ns, &bst_edges());
    assert!(r.positions.contains_key(&NodeId::new("loner")));
}

#[test]
fn width_and_height_grow_with_the_extent() {
    let r = tree(&bst_nodes(), &bst_edges());
    assert!(r.width > TREE_COL_W * 3.0);
    assert!(r.height > TREE_ROW_H * 2.0);
}

#[test]
fn a_cycle_does_not_hang() {
    let ns = ["a", "b"].map(node).to_vec();
    let es = vec![edge("a", "b", "left"), edge("b", "a", "left")];
    let r = tree(&ns, &es);
    assert_eq!(r.positions.len(), 2);
}
