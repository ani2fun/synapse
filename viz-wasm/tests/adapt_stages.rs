//! Stage-level behaviors (a focused port of the oracle's `AdaptStagesSpec` /
//! `HeapSnapshotSpec` / `HeapToGraphDiffSpec` cores — the 16 goldens carry the end-to-end
//! load; these pin the individual stage rules the goldens can't isolate).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use viz_wasm::engine::adapt::cleanup;
use viz_wasm::engine::adapt::diff;
use viz_wasm::engine::adapt::flow;
use viz_wasm::engine::adapt::projection::{ProjectedNode, ProjectedStep};
use viz_wasm::engine::adapt::rooting;
use viz_wasm::engine::adapt::segmentation;
use viz_wasm::engine::adapt::snapshot::HeapSnapshot;
use viz_wasm::engine::adapt::{self, VizError};
use viz_wasm::engine::graph::{NodeId, VizNode};
use viz_wasm::engine::trace::{ArrKind, HeapFrame, HeapObject, HeapScalar, HeapStep, HeapTrace, HeapValue};

// ── tiny builders ─────────────────────────────────────────────────────────────

fn int(v: i64) -> HeapValue {
    HeapValue::Scalar(HeapScalar::I(v))
}

fn sref(id: &str) -> HeapValue {
    HeapValue::Ref(id.to_owned())
}

fn node_obj(val: i64, next: Option<&str>) -> HeapObject {
    let mut fields = vec![("val".to_owned(), int(val))];
    if let Some(n) = next {
        fields.push(("next".to_owned(), sref(n)));
    }
    HeapObject::Instance {
        cls: "Node".to_owned(),
        fields,
    }
}

fn step(line: i32, locals: Vec<(&str, HeapValue)>, heap: Vec<(&str, HeapObject)>) -> HeapStep {
    HeapStep {
        line,
        event: "line".to_owned(),
        frames: vec![HeapFrame {
            fn_name: "solve".to_owned(),
            locals: locals.into_iter().map(|(n, v)| (n.to_owned(), v)).collect(),
        }],
        heap: heap.into_iter().map(|(id, o)| (id.to_owned(), o)).collect(),
    }
}

fn trace(steps: Vec<HeapStep>) -> HeapTrace {
    HeapTrace {
        steps,
        truncated: false,
    }
}

fn pstep(line: i32, ids: &[(&str, &str)]) -> ProjectedStep {
    ProjectedStep {
        line,
        event: "line".to_owned(),
        nodes: ids
            .iter()
            .map(|(id, label)| ProjectedNode {
                node: VizNode {
                    id: NodeId::new(*id),
                    label: (*label).to_owned(),
                    kind: "node".to_owned(),
                    ..VizNode::default()
                },
                owner: (*id).to_owned(),
            })
            .collect(),
        edges: Vec::new(),
        cursor: Vec::new(),
        frames: Vec::new(),
        structure_type: None,
    }
}

// ── cleanup ───────────────────────────────────────────────────────────────────

#[test]
fn cleanup_synthesises_a_hinted_string_as_char_cells_and_rebinds_the_local() {
    let t = trace(vec![step(
        1,
        vec![("s", HeapValue::Scalar(HeapScalar::S("ab".to_owned())))],
        vec![],
    )]);
    let cleaned = cleanup::clean(&t, Some("s"));
    let s0 = &cleaned.steps[0];
    let arr = s0.heap.get("__syn_str_s").unwrap();
    match arr {
        HeapObject::Arr { kind, items } => {
            assert_eq!(*kind, ArrKind::Lst);
            assert_eq!(items.len(), 2);
        }
        _ => panic!("expected an Arr"),
    }
    assert_eq!(s0.frames[0].locals[0].1, sref("__syn_str_s"));
}

#[test]
fn cleanup_drops_helper_frames_python_and_java() {
    let mut t = trace(vec![step(1, vec![], vec![]), step(2, vec![], vec![])]);
    t.steps[0].frames[0].fn_name = "__init__".to_owned();
    t.steps[1].frames[0].fn_name = "buildTree".to_owned();
    assert!(cleanup::clean(&t, None).steps.is_empty());
    // `total` must NOT be caught by the `to` prefix (the uppercase gate).
    let mut ok = trace(vec![step(1, vec![], vec![])]);
    ok.steps[0].frames[0].fn_name = "total".to_owned();
    assert_eq!(cleanup::clean(&ok, None).steps.len(), 1);
}

// ── snapshot ──────────────────────────────────────────────────────────────────

#[test]
fn reachability_is_preorder_and_memoized() {
    let heap: BTreeMap<String, HeapObject> = [
        ("a".to_owned(), node_obj(1, Some("b"))),
        ("b".to_owned(), node_obj(2, Some("c"))),
        ("c".to_owned(), node_obj(3, None)),
    ]
    .into();
    let snap = HeapSnapshot::new(&heap);
    assert_eq!(snap.reachable_from("a"), vec!["a", "b", "c"]);
    assert_eq!(snap.reachable_from("a"), vec!["a", "b", "c"]); // memo hit
}

#[test]
fn a_clrs_nil_sentinel_is_detected_and_a_normal_node_is_not() {
    let sentinel = HeapObject::Instance {
        cls: "Node".to_owned(),
        fields: vec![
            ("val".to_owned(), HeapValue::Scalar(HeapScalar::Null)),
            ("left".to_owned(), sref("nil")),
            ("right".to_owned(), sref("nil")),
        ],
    };
    let heap: BTreeMap<String, HeapObject> =
        [("nil".to_owned(), sentinel), ("n".to_owned(), node_obj(1, None))].into();
    let snap = HeapSnapshot::new(&heap);
    assert!(snap.is_null_sentinel("nil"));
    assert!(!snap.is_null_sentinel("n"));
}

// ── rooting ───────────────────────────────────────────────────────────────────

#[test]
fn rooting_prefers_the_hinted_local_then_attr_then_auto_detect() {
    let s = step(
        1,
        vec![("head", sref("h"))],
        vec![("h", node_obj(1, None)), ("x", node_obj(2, None))],
    );
    assert_eq!(
        rooting::resolve_root_id(std::slice::from_ref(&s), Some("head"), "list"),
        Some("h".to_owned())
    );
    // attr fallback: no local named `root`, but an instance carries the field.
    let s2 = step(
        1,
        vec![],
        vec![
            (
                "tree",
                HeapObject::Instance {
                    cls: "Tree".to_owned(),
                    fields: vec![("root".to_owned(), sref("r"))],
                },
            ),
            ("r", node_obj(5, None)),
        ],
    );
    assert_eq!(
        rooting::resolve_root_id(&[s2], Some("root"), "tree"),
        Some("r".to_owned())
    );
    // auto-detect: the unreferenced object with the biggest reachable set wins.
    let s3 = step(
        1,
        vec![],
        vec![
            ("a", node_obj(1, Some("b"))),
            ("b", node_obj(2, None)),
            ("z", node_obj(9, None)),
        ],
    );
    assert_eq!(rooting::resolve_root_id(&[s3], None, ""), Some("a".to_owned()));
}

#[test]
fn a_dotted_hint_follows_fields() {
    let s = step(
        1,
        vec![("self", sref("obj"))],
        vec![
            (
                "obj",
                HeapObject::Instance {
                    cls: "List".to_owned(),
                    fields: vec![("head".to_owned(), sref("h"))],
                },
            ),
            ("h", node_obj(1, None)),
        ],
    );
    assert_eq!(
        rooting::resolve_root_id(&[s], Some("self.head"), "list"),
        Some("h".to_owned())
    );
}

// ── segmentation ──────────────────────────────────────────────────────────────

#[test]
fn a_rebind_to_a_disconnected_structure_starts_a_new_case() {
    let case1 = step(1, vec![("head", sref("a"))], vec![("a", node_obj(1, None))]);
    let case2 = step(2, vec![("head", sref("z"))], vec![("z", node_obj(9, None))]);
    let cleaned = cleanup::clean(&trace(vec![case1, case2]), Some("head"));
    let segments = segmentation::segment(&cleaned, Some("head"), None);
    assert_eq!(segments.len(), 2);
}

#[test]
fn a_rebind_that_stays_connected_does_not_split() {
    // Recursive descent: head rebinds to the child, which the old root still reaches.
    let s1 = step(
        1,
        vec![("head", sref("a"))],
        vec![("a", node_obj(1, Some("b"))), ("b", node_obj(2, None))],
    );
    let s2 = step(
        2,
        vec![("head", sref("b"))],
        vec![("a", node_obj(1, Some("b"))), ("b", node_obj(2, None))],
    );
    let cleaned = cleanup::clean(&trace(vec![s1, s2]), Some("head"));
    assert_eq!(segmentation::segment(&cleaned, Some("head"), None).len(), 1);
}

// ── flow ──────────────────────────────────────────────────────────────────────

#[test]
fn drop_empty_ends_keeps_exactly_the_last_leading_blank() {
    let steps = vec![
        pstep(1, &[]),
        pstep(2, &[]),
        pstep(3, &[("a", "1")]),
        pstep(4, &[]),
    ];
    let out = flow::drop_empty_ends(steps);
    assert_eq!(out.iter().map(|s| s.line).collect::<Vec<_>>(), vec![2, 3]);
}

#[test]
fn carry_forward_fills_interior_blanks_keeping_their_own_line() {
    let steps = vec![pstep(1, &[("a", "1")]), pstep(2, &[]), pstep(3, &[("a", "2")])];
    let out = flow::carry_forward(steps);
    assert_eq!(out[1].line, 2);
    assert_eq!(out[1].nodes.len(), 1, "inherited the previous graph");
}

#[test]
fn coalesce_drops_exact_consecutive_duplicates() {
    let steps = vec![
        pstep(1, &[("a", "1")]),
        pstep(1, &[("a", "1")]),
        pstep(2, &[("a", "1")]),
    ];
    assert_eq!(flow::coalesce(steps).len(), 2);
}

// ── diff ──────────────────────────────────────────────────────────────────────

#[test]
fn diff_cues_highlight_changed_and_removed_reemitted_once() {
    let steps = vec![
        pstep(1, &[("a", "1"), ("b", "2")]),
        pstep(2, &[("a", "9"), ("c", "3")]), // a changed, b removed, c new
        pstep(3, &[("a", "9"), ("c", "3")]),
    ];
    let out = diff::diff(&steps).unwrap();
    assert!(out[0].highlight.is_empty(), "step 1 carries no cues");
    assert_eq!(out[1].highlight, vec![NodeId::new("c")]);
    assert_eq!(out[1].changed, vec![NodeId::new("a")]);
    assert_eq!(out[1].removed, vec![NodeId::new("b")]);
    assert_eq!(out[1].nodes.len(), 3, "the removed node is re-emitted this step");
    assert!(out[2].removed.is_empty(), "…and fades exactly once");
    assert_eq!(out[2].nodes.len(), 2);
}

#[test]
fn a_duplicate_node_id_is_a_loud_error() {
    let steps = vec![pstep(1, &[("a", "1"), ("a", "2")])];
    assert_eq!(
        diff::diff(&steps).unwrap_err(),
        VizError::DuplicateNodeId("a".to_owned())
    );
}

// ── the orchestrator's error surface ─────────────────────────────────────────

#[test]
fn empty_and_builder_only_traces_fail_typed() {
    assert_eq!(
        adapt::adapt(&trace(vec![]), "", "list", None, None, "").unwrap_err(),
        VizError::EmptyTrace
    );
    let mut t = trace(vec![step(1, vec![], vec![])]);
    t.steps[0].frames[0].fn_name = "__init__".to_owned();
    assert_eq!(
        adapt::adapt(&t, "", "list", None, None, "").unwrap_err(),
        VizError::OnlyBuilderFrames
    );
}

#[test]
fn the_callstack_route_projects_frames_as_a_growing_stack() {
    let mut s1 = step(1, vec![("n", int(3))], vec![]);
    s1.frames[0].fn_name = "fib".to_owned();
    let mut s2 = step(2, vec![("n", int(2))], vec![]);
    s2.frames = vec![
        HeapFrame {
            fn_name: "fib".to_owned(),
            locals: vec![("n".to_owned(), int(2))],
        },
        HeapFrame {
            fn_name: "fib".to_owned(),
            locals: vec![("n".to_owned(), int(3))],
        },
    ];
    let cases = adapt::adapt(&trace(vec![s1, s2]), "", "callstack", None, None, "t").unwrap();
    let steps = &cases.cases[0].steps;
    assert_eq!(steps[0].nodes.len(), 1);
    assert_eq!(steps[1].nodes.len(), 2, "the stack grew");
    assert_eq!(steps[0].nodes[0].label, "fib(3)");
    // Active frame on top: slot 1 is the innermost fib(2).
    let top = steps[1].nodes.iter().find(|n| n.slot == Some(1)).unwrap();
    assert_eq!(top.label, "fib(2)");
}

#[test]
fn narration_reads_initial_then_cursor_moves() {
    let arr = HeapObject::Arr {
        kind: ArrKind::Lst,
        items: vec![int(7), int(8)],
    };
    let s1 = step(
        1,
        vec![("arr", sref("A")), ("i", int(0))],
        vec![("A", arr.clone())],
    );
    let s2 = step(2, vec![("arr", sref("A")), ("i", int(1))], vec![("A", arr)]);
    let cases = adapt::adapt(&trace(vec![s1, s2]), "x\ny", "array", Some("arr"), None, "t").unwrap();
    let steps = &cases.cases[0].steps;
    // Step 0 already carries a cursor, so the oracle narrates ITS placement (moved-vs-empty
    // prev beats "initial structure" in the precedence — goldens confirm).
    assert_eq!(steps[0].annotation.title, "i moves to 7");
    assert_eq!(steps[1].annotation.title, "i moves to 8");
    assert!(!steps[1].cursor[0].color.is_empty(), "cursors are coloured last");

    // Without cursors, step 0 falls through to "initial structure".
    let bare1 = step(1, vec![("head", sref("a"))], vec![("a", node_obj(1, None))]);
    let bare = adapt::adapt(&trace(vec![bare1]), "x", "list", Some("head"), None, "t").unwrap();
    // `head` IS a ref cursor onto the node — so drop the local instead:
    let noc1 = step(1, vec![], vec![("a", node_obj(1, None))]);
    let noc = adapt::adapt(&trace(vec![noc1]), "x", "list", None, None, "t").unwrap();
    assert_eq!(noc.cases[0].steps[0].annotation.title, "initial structure");
    drop(bare);
}
