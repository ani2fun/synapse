//! Placed lists: real pointers drawn where they point, `∅` ends, cursors on null.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use super::*;
use crate::engine::graph::{VizEdge, VizField};

fn node(id: &str, col: i32, row: i32) -> VizNode {
    VizNode {
        id: NodeId::new(id),
        label: id.to_owned(),
        kind: "node".to_owned(),
        slot: Some(col),
        meta: if row == 0 {
            vec![]
        } else {
            vec![VizField {
                name: "row".to_owned(),
                value: row.to_string(),
            }]
        },
        ..VizNode::default()
    }
}

fn next(f: &str, t: &str) -> VizEdge {
    VizEdge {
        from: NodeId::new(f),
        to: NodeId::new(t),
        label: "next".to_owned(),
    }
}

fn cursor(name: &str, target: &str) -> VizCursor {
    VizCursor {
        name: name.to_owned(),
        target: NodeId::new(target),
        color: String::new(),
    }
}

fn graph(steps: Vec<VizStep>) -> VizGraph {
    VizGraph {
        steps,
        layout_hint: "placed".to_owned(),
        ..VizGraph::default()
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

#[test]
fn a_chain_draws_straight_arrows_and_one_null_end() {
    let s = step(
        vec![node("a", 0, 0), node("b", 1, 0), node("c", 2, 0)],
        vec![next("a", "b"), next("b", "c")],
        vec![],
    );
    let g = graph(vec![s.clone()]);
    let p = placement(&g);
    let f = p.frame(&s);
    assert_eq!(f.nulls.len(), 1);
    assert!(
        f.arrows.iter().all(|a| !a.d.contains('C')),
        "neighbours never arc"
    );
    assert!(!f.arc_above && !f.arc_below);
    // The `∅` sits after c, inside the drawing.
    assert!(f.nulls[0].x > f.boxes[2].at.x && f.nulls[0].x < p.width);
}

#[test]
fn a_skip_arcs_over_the_skipped_node_and_takes_headroom() {
    let s = step(
        vec![node("a", 0, 0), node("b", 1, 0), node("c", 2, 0)],
        vec![next("a", "c")],
        vec![],
    );
    let f = placement(&graph(vec![s.clone()])).frame(&s);
    assert!(f.arc_above);
    // b's next is null, and so is c's: two ends.
    assert_eq!(f.nulls.len(), 2);
}

#[test]
fn an_empty_column_between_neighbours_is_not_a_skip() {
    // 2 → 4 with the gap where 3 will go: one long straight arrow, as the book draws it.
    let s = step(
        vec![node("2", 0, 0), node("4", 2, 0)],
        vec![next("2", "4")],
        vec![],
    );
    let f = placement(&graph(vec![s.clone()])).frame(&s);
    assert!(!f.arc_above);
}

#[test]
fn a_node_above_the_row_points_down_into_its_target() {
    let s = step(
        vec![node("2", 0, 0), node("3", 1, -1), node("4", 2, 0)],
        vec![next("2", "4"), next("3", "4")],
        vec![],
    );
    let p = placement(&graph(vec![s.clone()]));
    let f = p.frame(&s);
    let three = f.boxes.iter().find(|b| b.node.id.value() == "3").unwrap().at;
    let four = f.boxes.iter().find(|b| b.node.id.value() == "4").unwrap().at;
    assert!(three.y < four.y);
    assert!(
        f.boxes.iter().all(|b| b.at.y > 0.0),
        "row -1 is inside the drawing"
    );
}

#[test]
fn opposite_neighbours_take_two_lanes() {
    let s = step(
        vec![node("a", 0, 0), node("b", 1, 0)],
        vec![next("a", "b"), next("b", "a")],
        vec![],
    );
    let f = placement(&graph(vec![s.clone()])).frame(&s);
    let ys: Vec<&str> = f.arrows.iter().map(|a| a.d.split(',').nth(1).unwrap()).collect();
    assert_ne!(ys[0].split(' ').next(), ys[1].split(' ').next());
}

#[test]
fn a_cycle_back_past_nodes_arcs_below() {
    let s = step(
        vec![node("a", 0, 0), node("b", 1, 0), node("c", 2, 0)],
        vec![next("a", "b"), next("b", "c"), next("c", "a")],
        vec![],
    );
    let f = placement(&graph(vec![s.clone()])).frame(&s);
    assert!(f.arc_below);
    assert!(f.nulls.is_empty(), "a cycle has no null end");
}

#[test]
fn a_null_cursor_sits_on_the_end_of_row_zero() {
    let s = step(
        vec![node("a", 0, 0), node("b", 1, 0), node("n", 1, -1)],
        vec![next("a", "b")],
        vec![cursor("curr", "null"), cursor("head", "a")],
    );
    let f = placement(&graph(vec![s.clone()])).frame(&s);
    let b = f.boxes.iter().find(|x| x.node.id.value() == "b").unwrap().at;
    let on_null = f.cursors.iter().find(|c| c.cursors[0].name == "curr").unwrap();
    assert_eq!(on_null.at.y, b.y);
    assert!(on_null.at.x > b.x);
}

#[test]
fn the_empty_list_draws_a_lone_null_for_its_head() {
    let s = step(vec![], vec![], vec![cursor("head", "null")]);
    let f = placement(&graph(vec![s.clone()])).frame(&s);
    assert_eq!(f.nulls.len(), 1);
    assert_eq!(f.cursors.len(), 1);
}

#[test]
fn doubly_lists_get_null_at_both_ends() {
    let prev = |f: &str, t: &str| VizEdge {
        from: NodeId::new(f),
        to: NodeId::new(t),
        label: "prev".to_owned(),
    };
    let s = step(
        vec![node("a", 0, 0), node("b", 1, 0)],
        vec![next("a", "b"), prev("b", "a")],
        vec![],
    );
    let p = placement(&graph(vec![s.clone()]));
    assert!(p.doubly);
    let f = p.frame(&s);
    assert_eq!(f.nulls.len(), 2);
    assert!(f.nulls.iter().all(|g| g.x > 0.0 && g.x < p.width));
}

#[test]
fn the_extent_covers_every_step() {
    let s1 = step(vec![node("a", 0, 0)], vec![], vec![]);
    let s2 = step(vec![node("a", 0, 0), node("b", 3, 1)], vec![], vec![]);
    let p = placement(&graph(vec![s1, s2.clone()]));
    let f = p.frame(&s2);
    let b = f.boxes[1].at;
    assert!(b.x + p.box_w() / 2.0 < p.width && b.y < p.height);
}
