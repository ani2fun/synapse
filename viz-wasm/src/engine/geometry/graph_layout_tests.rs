//! Oracle: `GraphLayoutSpec` — forest-vs-force dispatch and, load-bearing, DETERMINISM:
//! a redraw is byte-identical.

#![allow(clippy::unwrap_used)]

use super::*;

fn node(id: &str) -> VizNode {
    VizNode {
        id: NodeId::new(id),
        label: id.to_owned(),
        kind: "node".to_owned(),
        ..VizNode::default()
    }
}

fn edge(f: &str, t: &str) -> VizEdge {
    VizEdge {
        from: NodeId::new(f),
        to: NodeId::new(t),
        label: "edge".to_owned(),
    }
}

fn pos<'a>(r: &'a LayoutResult, id: &str) -> &'a Point {
    &r.positions[&NodeId::new(id)]
}

// A 3-cycle → force. A small tree → forest.
fn cycle_nodes() -> Vec<VizNode> {
    ["a", "b", "c"].map(node).to_vec()
}

fn cycle_edges() -> Vec<VizEdge> {
    vec![edge("a", "b"), edge("b", "c"), edge("c", "a")]
}

#[test]
fn a_redraw_is_byte_identical() {
    let a = graph(&cycle_nodes(), &cycle_edges());
    let b = graph(&cycle_nodes(), &cycle_edges());
    assert_eq!(a.positions, b.positions);
    assert_eq!((a.width, a.height), (b.width, b.height));
}

#[test]
fn a_bigger_cyclic_graph_is_also_deterministic() {
    let ns: Vec<VizNode> = (1..=8).map(|i| node(&format!("n{i}"))).collect();
    let es: Vec<VizEdge> = (1..=8)
        .map(|i| edge(&format!("n{i}"), &format!("n{}", i % 8 + 1)))
        .collect();
    assert_eq!(graph(&ns, &es).positions, graph(&ns, &es).positions);
}

#[test]
fn an_acyclic_single_parent_graph_lays_out_as_a_tidy_tree() {
    let ns = ["r", "x", "y"].map(node).to_vec();
    let es = vec![edge("r", "x"), edge("r", "y")];
    let r = graph(&ns, &es);
    assert!(pos(&r, "r").y <= pos(&r, "x").y);
    assert!(pos(&r, "r").y <= pos(&r, "y").y);
}

#[test]
fn a_forest_lays_out_under_a_dropped_synthetic_super_root() {
    let ns = ["r1", "a", "r2", "b"].map(node).to_vec();
    let es = vec![edge("r1", "a"), edge("r2", "b")];
    let r = graph(&ns, &es);
    for id in ["r1", "a", "r2", "b"] {
        assert!(r.positions.contains_key(&NodeId::new(id)), "{id}");
    }
    assert!(!r.positions.contains_key(&NodeId::new("__syn_superroot")));
}

#[test]
fn every_node_gets_a_position_in_the_cyclic_case() {
    let r = graph(&cycle_nodes(), &cycle_edges());
    assert!(cycle_nodes().iter().all(|n| r.positions.contains_key(&n.id)));
}

#[test]
fn all_positions_are_positive() {
    let r = graph(&cycle_nodes(), &cycle_edges());
    assert!(r.positions.values().all(|p| p.x >= 0.0 && p.y >= 0.0));
}

#[test]
fn width_and_height_bound_the_nodes() {
    let r = graph(&cycle_nodes(), &cycle_edges());
    assert!(r.width > 0.0 && r.height > 0.0);
    assert!(r.positions.values().all(|p| p.x <= r.width && p.y <= r.height));
}

#[test]
fn a_single_node_lays_out() {
    let r = graph(&[node("only")], &[]);
    assert!(r.positions.contains_key(&NodeId::new("only")));
}

#[test]
fn no_nodes_is_a_minimal_empty_layout() {
    let r = graph(&[], &[]);
    assert!(r.positions.is_empty());
    assert!(r.width > 0.0 && r.height > 0.0);
}

#[test]
fn force_spreads_a_cycles_nodes_apart() {
    let r = graph(&cycle_nodes(), &cycle_edges());
    let points: std::collections::HashSet<(u64, u64)> = cycle_nodes()
        .iter()
        .map(|n| {
            let p = pos(&r, n.id.value());
            (p.x.to_bits(), p.y.to_bits())
        })
        .collect();
    assert_eq!(points.len(), 3);
}

#[test]
fn a_disconnected_extra_node_in_a_cyclic_graph_is_placed() {
    let mut ns = cycle_nodes();
    ns.push(node("loner"));
    let r = graph(&ns, &cycle_edges());
    assert!(r.positions.contains_key(&NodeId::new("loner")));
}

#[test]
fn determinism_holds_with_a_disconnected_node_too() {
    let mut ns = cycle_nodes();
    ns.push(node("loner"));
    assert_eq!(
        graph(&ns, &cycle_edges()).positions,
        graph(&ns, &cycle_edges()).positions
    );
}
