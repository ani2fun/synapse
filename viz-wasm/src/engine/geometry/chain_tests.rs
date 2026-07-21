//! Oracle: `ChainLayoutSpec` — next-chain order, head detection, cycle/merge flags.

#![allow(clippy::unwrap_used, clippy::float_cmp)] // exact constants are the point

use super::super::constants::CHAIN_DX;
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

fn x_of(r: &LayoutResult, id: &str) -> f64 {
    r.positions[&NodeId::new(id)].x
}

#[test]
fn lays_out_the_next_chain_left_to_right_from_the_head() {
    let ns = ["a", "b", "c"].map(node).to_vec();
    let es = vec![edge("a", "b", "next"), edge("b", "c", "next")];
    let r = chain(&ns, &es).result;
    assert!(x_of(&r, "a") < x_of(&r, "b") && x_of(&r, "b") < x_of(&r, "c"));
    assert_eq!(x_of(&r, "b") - x_of(&r, "a"), CHAIN_DX);
}

#[test]
fn the_head_is_found_wherever_it_appears_in_input_order() {
    let ns = ["c", "b", "a"].map(node).to_vec(); // reversed input order
    let es = vec![edge("a", "b", "next"), edge("b", "c", "next")];
    let r = chain(&ns, &es).result;
    assert!(x_of(&r, "a") < x_of(&r, "b") && x_of(&r, "b") < x_of(&r, "c"));
}

#[test]
fn all_nodes_share_one_row() {
    let ns = ["a", "b"].map(node).to_vec();
    let r = chain(&ns, &[edge("a", "b", "next")]).result;
    let ys: std::collections::HashSet<u64> = r.positions.values().map(|p| p.y.to_bits()).collect();
    assert_eq!(ys.len(), 1);
}

#[test]
fn prev_edges_are_ignored_for_placement() {
    let ns = ["a", "b"].map(node).to_vec();
    let es = vec![edge("a", "b", "next"), edge("b", "a", "prev")];
    let cl = chain(&ns, &es);
    assert!(x_of(&cl.result, "a") < x_of(&cl.result, "b"));
    assert!(!cl.graph_fallback);
}

#[test]
fn a_node_off_the_chain_is_appended() {
    let ns = ["a", "b", "loner"].map(node).to_vec();
    let r = chain(&ns, &[edge("a", "b", "next")]).result;
    assert!(r.positions.contains_key(&NodeId::new("loner")));
}

#[test]
fn a_cycle_stops_the_walk_and_flags_graph_fallback() {
    let ns = ["a", "b"].map(node).to_vec();
    let es = vec![edge("a", "b", "next"), edge("b", "a", "next")];
    let cl = chain(&ns, &es);
    assert!(cl.graph_fallback);
    assert_eq!(cl.result.positions.len(), 2);
}

#[test]
fn a_merge_flags_graph_fallback() {
    let ns = ["a", "b", "c"].map(node).to_vec();
    let es = vec![edge("a", "c", "next"), edge("b", "c", "next")];
    assert!(chain(&ns, &es).graph_fallback);
}

#[test]
fn width_grows_with_the_chain_length() {
    let short = chain(&[node("a")], &[]).result;
    let long = chain(
        &["a", "b", "c"].map(node),
        &[edge("a", "b", "next"), edge("b", "c", "next")],
    )
    .result;
    assert!(long.width > short.width);
}
