//! The d2 export, family by family. These assert the SOURCE, not a picture: whether that source
//! draws is `dev-tools/d2-viz-exports-compile.mjs`, which compiles the same fixtures through the
//! real engine. Rust cannot tell valid d2 from a plausible-looking string.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::engine::graph::{Annotation, VizCursor, VizEdge, VizStep};

fn cell(id: &str, label: &str, slot: i32) -> VizNode {
    VizNode {
        id: NodeId::new(id),
        label: label.to_owned(),
        kind: "cell".to_owned(),
        slot: Some(slot),
        ..VizNode::default()
    }
}

fn node(id: &str, label: &str) -> VizNode {
    VizNode {
        id: NodeId::new(id),
        label: label.to_owned(),
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

fn cursor(name: &str, target: &str) -> VizCursor {
    VizCursor {
        name: name.to_owned(),
        target: NodeId::new(target),
        color: String::new(),
    }
}

fn graph(title: &str, steps: Vec<VizStep>) -> VizGraph {
    VizGraph {
        steps,
        title: title.to_owned(),
        ..VizGraph::default()
    }
}

fn array_step() -> VizStep {
    VizStep {
        nodes: vec![cell("a", "5", 0), cell("b", "2", 1), cell("c", "8", 2)],
        cursor: vec![cursor("left", "a"), cursor("right", "c")],
        changed: vec![NodeId::new("b")],
        annotation: Annotation {
            body: "swap arr[left] and arr[right]".to_owned(),
            ..Annotation::default()
        },
        ..VizStep::default()
    }
}

// ── the cell families ─────────────────────────────────────────────────────────

#[test]
fn an_array_is_a_two_row_grid_of_values_then_indices() {
    let out = step_source(&graph("arr", vec![array_step()]), VizStructure::Array, 0);
    assert!(out.contains("arr: \"arr\" {"), "{out}");
    assert!(out.contains("grid-rows: 2"), "{out}");
    assert!(out.contains("grid-gap: 0"), "{out}");
    // Values first, indices second: d2 fills row-major in declaration order, and that ordering
    // is the whole reason the index rail lands under its own value.
    let values = out.find("c0: \"5\"").unwrap();
    let indices = out.find("i0: 0").unwrap();
    assert!(
        values < indices,
        "the index rail must be declared after the values:\n{out}"
    );
}

#[test]
fn a_cursor_is_an_arrow_into_the_grid_cell_never_a_near_key() {
    let out = step_source(&graph("arr", vec![array_step()]), VizStructure::Array, 0);
    assert!(out.contains("p0: \"left\""), "{out}");
    assert!(out.contains("p0 -> arr.c0:"), "{out}");
    assert!(out.contains("p1 -> arr.c2:"), "{out}");
    // d2 REFUSES `near` on a grid cell ("near keys cannot be set to descendants of special
    // objects"), so an export that reached for it would not compile at all.
    assert!(!out.contains("near: arr"), "{out}");
}

#[test]
fn the_diff_cue_rides_the_cell_it_belongs_to() {
    let out = step_source(&graph("arr", vec![array_step()]), VizStructure::Array, 0);
    let changed = out.lines().find(|l| l.trim_start().starts_with("c1:")).unwrap();
    assert!(changed.contains("style.stroke"), "{changed}");
    let plain = out.lines().find(|l| l.trim_start().starts_with("c0:")).unwrap();
    assert!(!plain.contains("style."), "{plain}");
}

#[test]
fn a_stack_is_one_column_and_carries_no_index_rail() {
    let out = step_source(&graph("st", vec![array_step()]), VizStructure::Stack, 0);
    assert!(out.contains("grid-columns: 1"), "{out}");
    // A stack is addressed by its top, not by position — an index rail would double its width
    // to say nothing.
    assert!(!out.contains("i0: 0"), "{out}");
}

#[test]
fn the_caption_is_a_text_shape_pinned_to_a_constant_position() {
    let out = step_source(&graph("arr", vec![array_step()]), VizStructure::Array, 0);
    assert!(
        out.starts_with("caption: \"swap arr[left] and arr[right]\""),
        "{out}"
    );
    // `near: top-center` is a CONSTANT, which is legal on a board whose figure is a grid.
    assert!(out.contains("near: top-center"), "{out}");
}

// ── the grid ──────────────────────────────────────────────────────────────────

#[test]
fn a_ragged_grid_keeps_its_holes() {
    // A grid arrives as outer ref-cells pointing at inner rows; a short row leaves a hole.
    let outer = |id: &str, slot: i32| VizNode {
        id: NodeId::new(id),
        label: "·".to_owned(),
        kind: "cell".to_owned(),
        slot: Some(slot),
        ..VizNode::default()
    };
    let step = VizStep {
        nodes: vec![
            outer("r0", 0),
            outer("r1", 1),
            cell("a", "1", 0),
            cell("b", "2", 1),
            cell("c", "3", 0),
        ],
        edges: vec![edge("r0", "a", ""), edge("r0", "b", ""), edge("r1", "c", "")],
        ..VizStep::default()
    };
    let out = step_source(&graph("g", vec![step]), VizStructure::Grid, 0);
    assert!(out.contains("grid-columns: 2"), "{out}");
    assert!(
        out.contains("c1_1: \"\""),
        "the short row's hole must stay a cell:\n{out}"
    );
    assert!(out.contains("style.stroke-dash"), "{out}");
}

// ── the node/edge families ────────────────────────────────────────────────────

#[test]
fn a_tree_is_nodes_and_labelled_edges() {
    let step = VizStep {
        nodes: vec![node("a", "8"), node("b", "3"), node("c", "10")],
        edges: vec![edge("a", "b", "left"), edge("a", "c", "right")],
        ..VizStep::default()
    };
    let out = step_source(&graph("t", vec![step]), VizStructure::Tree, 0);
    assert!(out.contains("direction: down"), "{out}");
    assert!(out.contains("n0 -> n1: \"left\""), "{out}");
    assert!(out.contains("n0 -> n2: \"right\""), "{out}");
}

#[test]
fn a_list_walks_next_and_ends_at_null() {
    let step = VizStep {
        nodes: vec![node("a", "1"), node("b", "2")],
        edges: vec![edge("a", "b", "next")],
        ..VizStep::default()
    };
    let out = step_source(&graph("l", vec![step]), VizStructure::List, 0);
    assert!(out.contains("n0 -> n1: next"), "{out}");
    assert!(
        out.contains("n1 -> null: next"),
        "the tail must visibly be the tail:\n{out}"
    );
}

#[test]
fn an_empty_structure_says_so_rather_than_emitting_nothing() {
    let out = step_source(&graph("arr", vec![VizStep::default()]), VizStructure::Array, 0);
    assert!(out.contains("(empty)"), "{out}");
    // An empty document would compile and draw a blank page, which reads as a broken export.
    assert!(!out.trim().is_empty());
}

// ── the walkthrough ───────────────────────────────────────────────────────────

fn changing(n: usize) -> Vec<VizStep> {
    (0..n)
        .map(|i| VizStep {
            nodes: vec![cell("a", &i.to_string(), 0)],
            unchanged: false,
            ..VizStep::default()
        })
        .collect()
}

#[test]
fn a_walkthrough_is_one_layer_per_changing_step() {
    let mut steps = changing(3);
    steps[1].unchanged = true;
    let out = walkthrough_source(&graph("arr", steps), VizStructure::Array);
    assert!(out.contains("layers: {"), "{out}");
    assert!(out.contains("step-1: {"), "{out}");
    assert!(out.contains("step-2: {"), "{out}");
    // Three steps, one of them a line-only advance → two boards.
    assert!(
        !out.contains("step-3: {"),
        "an unchanged step must not become a board:\n{out}"
    );
}

#[test]
fn a_walkthrough_is_capped_because_it_is_a_figure_not_a_movie() {
    let out = walkthrough_source(&graph("arr", changing(40)), VizStructure::Array);
    assert!(out.contains(&format!("step-{MAX_BOARDS}: {{")), "{out}");
    assert!(!out.contains(&format!("step-{}: {{", MAX_BOARDS + 1)), "{out}");
}

#[test]
fn a_trace_that_changes_at_every_line_still_yields_boards() {
    // Every step marked `unchanged` would otherwise leave a walkthrough of one board.
    let steps: Vec<VizStep> = changing(3)
        .into_iter()
        .map(|mut s| {
            s.unchanged = true;
            s
        })
        .collect();
    let out = walkthrough_source(&graph("arr", steps), VizStructure::Array);
    assert!(out.contains("step-3: {"), "{out}");
}

// ── the mechanics ─────────────────────────────────────────────────────────────

#[test]
fn a_label_with_a_quote_survives_as_one_label() {
    let step = VizStep {
        nodes: vec![cell("a", "say \"hi\"", 0)],
        ..VizStep::default()
    };
    let out = step_source(&graph("arr", vec![step]), VizStructure::Array, 0);
    assert!(out.contains(r#"c0: "say \"hi\"""#), "{out}");
}

#[test]
fn a_prose_title_becomes_the_label_and_never_the_key() {
    let step = VizStep {
        nodes: vec![cell("a", "1", 0)],
        ..VizStep::default()
    };
    let out = step_source(
        &graph("Array — two-pointer reverse of [1, 2, 3]", vec![step]),
        VizStructure::Array,
        0,
    );
    // The key is what every cursor arrow and every hand edit has to type.
    assert!(
        out.starts_with("arr: \"Array — two-pointer reverse of [1, 2, 3]\" {"),
        "{out}"
    );
}

#[test]
fn a_step_past_the_end_is_empty_rather_than_a_panic() {
    assert_eq!(step_source(&graph("arr", vec![]), VizStructure::Array, 3), "");
}
