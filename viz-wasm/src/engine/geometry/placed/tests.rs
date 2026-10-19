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

fn region(id: &str, col: i32, row: i32, cols: f64, rows: f64, label: &str, grid: bool) -> VizNode {
    let mut meta = vec![
        VizField {
            name: "cols".to_owned(),
            value: cols.to_string(),
        },
        VizField {
            name: "rows".to_owned(),
            value: rows.to_string(),
        },
        VizField {
            name: "label".to_owned(),
            value: label.to_owned(),
        },
    ];
    if row != 0 {
        meta.push(VizField {
            name: "row".to_owned(),
            value: row.to_string(),
        });
    }
    if grid {
        meta.push(VizField {
            name: "grid".to_owned(),
            value: "true".to_owned(),
        });
    }
    VizNode {
        id: NodeId::new(id),
        kind: REGION.to_owned(),
        slot: Some(col),
        meta,
        ..VizNode::default()
    }
}

#[test]
fn a_region_is_a_frame_not_a_node() {
    let one = step(
        vec![
            region("mem", 0, 0, 4.0, 3.0, "Memory", true),
            node("a", 1, 1),
            node("b", 3, 2),
        ],
        vec![next("a", "b")],
        vec![],
    );
    let placed = placement(&graph(vec![one.clone()]));
    let frame = placed.frame(&one);
    assert_eq!(frame.boxes.len(), 2, "the region draws no box");
    assert_eq!(frame.nulls.len(), 1, "and gets no null end");
    let reg = &frame.regions[0];
    assert_eq!(
        reg.lines.len(),
        3 + 2,
        "3 inner column lines and 2 inner row lines"
    );
    // Every node sits inside its cell, and the frame and its label are inside the drawing.
    for bx in &frame.boxes {
        assert!(bx.at.x > reg.x && bx.at.x < reg.x + reg.w && bx.at.y > reg.y && bx.at.y < reg.y + reg.h);
    }
    assert!(
        reg.x >= 0.0
            && reg.y >= 0.0
            && reg.x + reg.w <= placed.width
            && reg.y + reg.h + REGION_LABEL_DY <= placed.height
    );
}

#[test]
fn a_cross_row_arrow_bends_round_a_node_in_its_way() {
    // b → c runs from the bottom-left to the top-right, and a sits right on the straight line.
    let one = step(
        vec![node("b", 0, 2), node("a", 1, 1), node("c", 2, 0)],
        vec![next("b", "c")],
        vec![],
    );
    let placed = placement(&graph(vec![one.clone()]));
    let frame = placed.frame(&one);
    let blocker = frame.boxes.iter().find(|x| x.node.id.value() == "a").unwrap().at;
    let arrow = frame.arrows.iter().find(|x| x.d.contains('C')).unwrap();
    let curve: [Point; 4] = arrow.pts.clone().try_into().unwrap();
    let hw = placed.box_w() / 2.0;
    for i in 0..=40 {
        let point = cubic(&curve, f64::from(i) / 40.0);
        assert!(
            (point.x - blocker.x).abs() >= hw || (point.y - blocker.y).abs() >= LIST_BOX_H / 2.0,
            "the curve crosses node a at {point:?}"
        );
    }
}

#[test]
fn the_hint_sets_the_row_pitch() {
    let one = step(vec![node("a", 0, 0), node("b", 0, 1)], vec![], vec![]);
    let mut gr = graph(vec![one.clone()]);
    gr.layout_hint = "placed dy=70".to_owned();
    assert!(is_placed(&gr.layout_hint));
    let frame = placement(&gr).frame(&one);
    assert!((frame.boxes[1].at.y - frame.boxes[0].at.y - 70.0).abs() < 1e-9);
}

#[test]
fn a_cross_row_arrow_keeps_clear_of_a_pointer_name() {
    // c is straight above d; d carries a pointer name, so the arrow may not drop onto it from above.
    let one = step(
        vec![node("c", 0, 0), node("d", 0, 3)],
        vec![next("c", "d")],
        vec![cursor("tail", "d")],
    );
    let placed = placement(&graph(vec![one.clone()]));
    let frame = placed.frame(&one);
    let target = frame.boxes.iter().find(|x| x.node.id.value() == "d").unwrap().at;
    let arrow = frame.arrows.iter().find(|x| x.d.contains('C')).unwrap();
    let curve: [Point; 4] = arrow.pts.clone().try_into().unwrap();
    for i in 0..=40 {
        let point = cubic(&curve, f64::from(i) / 40.0);
        let name = Point {
            x: target.x,
            y: target.y - LIST_BOX_H / 2.0 - 18.0,
        };
        assert!(
            (point.x - name.x).abs() >= 24.0 || (point.y - name.y).abs() >= 16.0,
            "the curve crosses the name at {point:?}"
        );
    }
}

#[test]
fn a_skip_arc_runs_under_the_row_when_a_spanned_node_has_a_pointer_name() {
    let one = step(
        vec![node("a", 0, 0), node("b", 1, 0), node("c", 2, 0)],
        vec![next("a", "c"), next("b", "c")],
        vec![cursor("current", "b")],
    );
    let frame = placement(&graph(vec![one.clone()])).frame(&one);
    let skip = frame.arrows.iter().find(|x| x.d.contains('C')).unwrap();
    let mid = frame.boxes.iter().find(|x| x.node.id.value() == "b").unwrap().at;
    let curve: [Point; 4] = skip.pts.clone().try_into().unwrap();
    assert!(frame.arc_below && !frame.arc_above);
    assert!(
        cubic(&curve, 0.5).y > mid.y + LIST_BOX_H / 2.0,
        "the arc should pass under the row"
    );
}
