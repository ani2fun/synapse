//! Stage 4 (oracle: `HeapProjection.scala`): one rooted segment → drawable steps. Per step:
//! re-resolve the root (rotation guard), take the reachable set minus null sentinels, project
//! instances/arrays/dicts to nodes + edges, group cards and infer a per-card layout kind. The
//! authored layout override applies to the ROOT CARD ONLY (ADR-S030 delta — the oracle forced
//! every card; the fixtures are single-forced-card, so goldens don't move).

use std::collections::{BTreeMap, HashSet};

use crate::engine::adapt::cards;
use crate::engine::adapt::cursors;
use crate::engine::adapt::rooting::{self, RootedSegment};
use crate::engine::adapt::snapshot::HeapSnapshot;
use crate::engine::adapt::vocab;
use crate::engine::graph::{NodeId, VizCursor, VizEdge, VizField, VizFrame, VizLocal, VizNode};
use crate::engine::trace::{HeapObject, HeapScalar, HeapStep, HeapValue};

/// A drawable node before diffing, carrying its owning heap id STRUCTURALLY (no id parsing —
/// the oracle's `takeWhile(_ != '#')` wart, prevented).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedNode {
    pub node: VizNode,
    pub owner: String,
}

/// One step projected to drawable form — pre-diff.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedStep {
    pub line: i32,
    pub event: String,
    pub nodes: Vec<ProjectedNode>,
    pub edges: Vec<VizEdge>,
    pub cursor: Vec<VizCursor>,
    pub frames: Vec<VizFrame>,
    pub structure_type: Option<String>,
}

#[must_use]
pub fn project(rooted: &RootedSegment, root_hint: Option<&str>, layout_hint: &str) -> Vec<ProjectedStep> {
    let layout_override = vocab::is_known_layout_kind(layout_hint).then(|| layout_hint.to_owned());
    match &rooted.root_id {
        None => rooted
            .steps
            .iter()
            .map(|s| ProjectedStep {
                line: s.line,
                event: s.event.clone(),
                nodes: Vec::new(),
                edges: Vec::new(),
                cursor: Vec::new(),
                frames: build_frames(s),
                structure_type: None,
            })
            .collect(),
        Some(root_id) => rooted
            .steps
            .iter()
            .map(|step| build_step(step, root_id, root_hint, layout_override.as_deref()))
            .collect(),
    }
}

fn build_step(
    step: &HeapStep,
    root_id: &str,
    root_hint: Option<&str>,
    layout_override: Option<&str>,
) -> ProjectedStep {
    let heap = &step.heap;
    let snap = HeapSnapshot::new(heap);
    let step_root = rooting::step_root_for(step, root_hint, root_id);
    let reachable: Vec<String> = snap
        .reachable_from(&step_root)
        .into_iter()
        .filter(|id| !snap.is_null_sentinel(id))
        .collect();

    let projected: Vec<ProjectedNode> = reachable
        .iter()
        .filter_map(|id| heap.get(id).map(|o| nodes_of(id, o, heap)))
        .flatten()
        .collect();
    let node_ids: HashSet<String> = projected.iter().map(|pn| pn.node.id.value().to_owned()).collect();
    let edges: Vec<VizEdge> = reachable
        .iter()
        .filter_map(|id| heap.get(id).map(|o| edges_of(id, o, heap, &node_ids)))
        .flatten()
        .collect();

    let card_by_obj = cards::group_cards(&reachable, heap);
    let root_card = card_by_obj.get(&step_root).cloned();
    let layout_by_card = infer_layout_kinds(
        &reachable,
        heap,
        &card_by_obj,
        layout_override,
        root_card.as_deref(),
    );

    let nodes: Vec<ProjectedNode> = projected
        .into_iter()
        .map(|pn| {
            let card = card_by_obj
                .get(&pn.owner)
                .cloned()
                .unwrap_or_else(|| pn.owner.clone());
            let layout_kind = layout_by_card.get(&card).cloned().unwrap_or_default();
            ProjectedNode {
                node: VizNode {
                    card_id: card,
                    layout_kind,
                    ..pn.node
                },
                owner: pn.owner,
            }
        })
        .collect();
    ProjectedStep {
        line: step.line,
        event: step.event.clone(),
        cursor: cursors::cursors(step, &step_root, &node_ids),
        frames: build_frames(step),
        nodes,
        edges,
        structure_type: None,
    }
}

// ── nodes ──
fn nodes_of(id: &str, obj: &HeapObject, heap: &BTreeMap<String, HeapObject>) -> Vec<ProjectedNode> {
    match obj {
        HeapObject::Instance { cls, fields } => {
            let (label, meta) = node_view(cls, fields);
            vec![ProjectedNode {
                node: VizNode {
                    id: NodeId::new(id),
                    label,
                    kind: cls.clone(),
                    meta,
                    ..VizNode::default()
                },
                owner: id.to_owned(),
            }]
        }
        HeapObject::Arr { items, .. } => items
            .iter()
            .enumerate()
            .map(|(i, item)| ProjectedNode {
                node: VizNode {
                    id: NodeId::new(format!("{id}#{i}")),
                    label: value_label(item),
                    kind: "cell".to_owned(),
                    slot: Some(i32::try_from(i).unwrap_or(i32::MAX)),
                    ..VizNode::default()
                },
                owner: id.to_owned(),
            })
            .collect(),
        HeapObject::Dict { entries } => entries
            .iter()
            .map(|(k, v)| ProjectedNode {
                node: VizNode {
                    id: NodeId::new(format!("{id}#{}", key_id(k))),
                    label: value_label(v),
                    kind: "entry".to_owned(),
                    meta: vec![VizField {
                        name: "key".to_owned(),
                        value: key_display(k, heap),
                    }],
                    ..VizNode::default()
                },
                owner: id.to_owned(),
            })
            .collect(),
    }
}

fn node_ids_of(id: &str, heap: &BTreeMap<String, HeapObject>) -> Vec<String> {
    match heap.get(id) {
        Some(HeapObject::Instance { .. }) => vec![id.to_owned()],
        Some(HeapObject::Arr { items, .. }) => (0..items.len()).map(|i| format!("{id}#{i}")).collect(),
        Some(HeapObject::Dict { entries }) => entries
            .iter()
            .map(|(k, _)| format!("{id}#{}", key_id(k)))
            .collect(),
        None => Vec::new(),
    }
}

// Primary value field → its scalar label · a ref → `·` · no value field → the class name.
// Meta = the non-value scalar fields; a MULTI-scalar class also re-lists the value field by
// name, a single-scalar class does not.
fn node_view(cls: &str, fields: &[(String, HeapValue)]) -> (String, Vec<VizField>) {
    let value_field = vocab::VALUE_FIELDS
        .iter()
        .find(|vf| fields.iter().any(|(n, _)| n == *vf))
        .copied();
    let primary = value_field.and_then(|vf| fields.iter().find(|(n, _)| n == vf).map(|(_, v)| v));
    let label = match primary {
        Some(HeapValue::Scalar(s)) => scalar_label(s),
        Some(HeapValue::Ref(_)) => vocab::REF_LABEL.to_owned(),
        None => cls.to_owned(),
    };
    let non_value_scalars: Vec<VizField> = fields
        .iter()
        .filter_map(|(name, v)| match v {
            HeapValue::Scalar(s) if value_field != Some(name.as_str()) && *s != HeapScalar::Null => {
                Some(VizField {
                    name: name.clone(),
                    value: scalar_label(s),
                })
            }
            _ => None,
        })
        .collect();
    let meta = if non_value_scalars.is_empty() {
        Vec::new()
    } else {
        let primary_field = value_field.and_then(|vf| {
            fields.iter().find_map(|(n, v)| match v {
                HeapValue::Scalar(s) if n == vf => Some(VizField {
                    name: vf.to_owned(),
                    value: scalar_label(s),
                }),
                _ => None,
            })
        });
        primary_field.into_iter().chain(non_value_scalars).collect()
    };
    (label, meta)
}

/// Scala `Double.toString` parity for the integral case (`1.0`, not `1`).
fn scala_double_label(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{v:.1}")
    } else {
        v.to_string()
    }
}

pub(crate) fn scalar_label(s: &HeapScalar) -> String {
    match s {
        HeapScalar::I(v) => v.to_string(),
        HeapScalar::D(v) => scala_double_label(*v),
        HeapScalar::B(v) => v.to_string(),
        HeapScalar::S(v) => v.clone(),
        HeapScalar::Null => "null".to_owned(),
    }
}

fn value_label(v: &HeapValue) -> String {
    match v {
        HeapValue::Scalar(s) => scalar_label(s),
        HeapValue::Ref(_) => vocab::REF_LABEL.to_owned(),
    }
}

fn key_id(k: &HeapValue) -> String {
    match k {
        HeapValue::Scalar(s) => scalar_label(s),
        HeapValue::Ref(id) => format!("@{id}"),
    }
}

fn key_display(k: &HeapValue, heap: &BTreeMap<String, HeapObject>) -> String {
    match k {
        HeapValue::Scalar(s) => scalar_label(s),
        HeapValue::Ref(id) => match heap.get(id) {
            Some(HeapObject::Instance { cls, fields }) => node_view(cls, fields).0,
            _ => vocab::REF_LABEL.to_owned(),
        },
    }
}

// ── edges ──
fn edges_of(
    id: &str,
    obj: &HeapObject,
    heap: &BTreeMap<String, HeapObject>,
    node_ids: &HashSet<String>,
) -> Vec<VizEdge> {
    let edges_to = |from: &str, to: &str, label: &str| -> Vec<VizEdge> {
        node_ids_of(to, heap)
            .into_iter()
            .map(|t| VizEdge {
                from: NodeId::new(from),
                to: NodeId::new(t),
                label: label.to_owned(),
            })
            .filter(|e| node_ids.contains(e.from.value()) && node_ids.contains(e.to.value()))
            .collect()
    };
    match obj {
        HeapObject::Instance { fields, .. } => fields
            .iter()
            .flat_map(|(field, v)| match v {
                HeapValue::Ref(to) => edges_to(id, to, field),
                HeapValue::Scalar(_) => Vec::new(),
            })
            .collect(),
        HeapObject::Arr { items, .. } => items
            .iter()
            .enumerate()
            .flat_map(|(i, v)| match v {
                HeapValue::Ref(to) => edges_to(&format!("{id}#{i}"), to, ""),
                HeapValue::Scalar(_) => Vec::new(),
            })
            .collect(),
        HeapObject::Dict { entries } => entries
            .iter()
            .flat_map(|(k, v)| {
                let entry_id = format!("{id}#{}", key_id(k));
                let key_edges = match k {
                    HeapValue::Ref(to) => edges_to(&entry_id, to, "key"),
                    HeapValue::Scalar(_) => Vec::new(),
                };
                let value_edges = match v {
                    HeapValue::Ref(to) => edges_to(&entry_id, to, &key_display(k, heap)),
                    HeapValue::Scalar(_) => Vec::new(),
                };
                key_edges.into_iter().chain(value_edges)
            })
            .collect(),
    }
}

// ── per-card layout kind (root card takes the override; others always infer) ──
fn infer_layout_kinds(
    reachable: &[String],
    heap: &BTreeMap<String, HeapObject>,
    card_by_obj: &std::collections::HashMap<String, String>,
    layout_override: Option<&str>,
    root_card: Option<&str>,
) -> std::collections::HashMap<String, String> {
    let card_ids: HashSet<&String> = reachable.iter().filter_map(|id| card_by_obj.get(id)).collect();
    card_ids
        .into_iter()
        .map(|card_id| {
            let inferred = infer_one_card(card_id, heap);
            let kind = if root_card == Some(card_id.as_str()) {
                layout_override.map_or(inferred, str::to_owned)
            } else {
                inferred
            };
            (card_id.clone(), kind)
        })
        .collect()
}

fn infer_one_card(card_id: &str, heap: &BTreeMap<String, HeapObject>) -> String {
    match heap.get(card_id) {
        Some(HeapObject::Arr { items, .. }) => {
            let all_arr_items = !items.is_empty()
                && items.iter().all(|v| match v {
                    HeapValue::Ref(id) => matches!(heap.get(id), Some(HeapObject::Arr { .. })),
                    HeapValue::Scalar(_) => false,
                });
            if all_arr_items { "array-2d" } else { "array-1d" }.to_owned()
        }
        Some(HeapObject::Dict { .. }) => "hashmap".to_owned(),
        Some(HeapObject::Instance { fields, .. }) => {
            let names: HashSet<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
            if names.contains("left") && names.contains("right") {
                "tree-binary"
            } else if names.contains("next") && names.contains("prev") {
                "list-double"
            } else if names.contains("next") {
                "list-single"
            } else {
                "graph-generic"
            }
            .to_owned()
        }
        None => "graph-generic".to_owned(),
    }
}

// ── frames panel ──
// Helper/constructor frames and the `self` local are dropped; `is_active` is the innermost
// surviving frame. Changed-flags are stamped later by StepDiff.
pub(crate) fn build_frames(step: &HeapStep) -> Vec<VizFrame> {
    step.frames
        .iter()
        .filter(|f| !vocab::is_helper_frame(&f.fn_name))
        .enumerate()
        .map(|(i, f)| VizFrame {
            fn_name: f.fn_name.clone(),
            locals: f
                .locals
                .iter()
                .filter(|(n, _)| n != "self")
                .map(|(n, v)| VizLocal {
                    name: n.clone(),
                    type_name: type_label(v, &step.heap),
                    value: value_display(v, &step.heap),
                    changed: false,
                })
                .collect(),
            is_active: i == 0,
        })
        .collect()
}

fn type_label(v: &HeapValue, heap: &BTreeMap<String, HeapObject>) -> String {
    match v {
        HeapValue::Scalar(HeapScalar::I(_)) => "int".to_owned(),
        HeapValue::Scalar(HeapScalar::D(_)) => "float".to_owned(),
        HeapValue::Scalar(HeapScalar::B(_)) => "bool".to_owned(),
        HeapValue::Scalar(HeapScalar::S(_)) => "str".to_owned(),
        HeapValue::Scalar(HeapScalar::Null) => "None".to_owned(),
        HeapValue::Ref(id) => match heap.get(id) {
            Some(HeapObject::Instance { cls, .. }) => cls.clone(),
            Some(HeapObject::Arr { .. }) => "list".to_owned(),
            Some(HeapObject::Dict { .. }) => "dict".to_owned(),
            None => "?".to_owned(),
        },
    }
}

// A frame local's display value: scalar inline · instance → its node label · a list/dict → a
// preview (first 12 elements, `…` past that — the frames panel has the row width, and the
// view CSS ellipsizes whatever still overflows; deliberate divergence from the oracle's
// 3-element preview, user ask 2026-07-17) · a dangling ref → `?`.
const ARR_PREVIEW_ITEMS: usize = 12;

fn value_display(v: &HeapValue, heap: &BTreeMap<String, HeapObject>) -> String {
    match v {
        HeapValue::Scalar(s) => scalar_label(s),
        HeapValue::Ref(id) => match heap.get(id) {
            Some(HeapObject::Instance { cls, fields }) => node_view(cls, fields).0,
            Some(HeapObject::Arr { items, .. }) => {
                let preview: Vec<String> = items.iter().take(ARR_PREVIEW_ITEMS).map(value_label).collect();
                let ellipsis = if items.len() > ARR_PREVIEW_ITEMS {
                    ", …"
                } else {
                    ""
                };
                format!("[{}{ellipsis}]", preview.join(", "))
            }
            Some(HeapObject::Dict { entries }) => {
                if entries.len() == 1 {
                    "{1 entry}".to_owned()
                } else {
                    format!("{{{} entries}}", entries.len())
                }
            }
            None => "?".to_owned(),
        },
    }
}
