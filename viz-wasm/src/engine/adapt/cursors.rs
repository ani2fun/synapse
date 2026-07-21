//! Frame locals that point at nodes. Two kinds, both from
//! the ACTIVE frame's locals: index cursors (an integer local whose name is in the allowlist
//! and whose value indexes the root array) and ref cursors (a local holding a reference to a
//! rendered node). Colours are assigned later (narration); `color` stays empty here.

use std::collections::HashSet;

use crate::engine::adapt::rooting;
use crate::engine::adapt::vocab;
use crate::engine::graph::{NodeId, VizCursor};
use crate::engine::trace::{HeapObject, HeapScalar, HeapStep, HeapValue};

#[must_use]
#[allow(clippy::implicit_hasher)] // one call site, std hasher
pub fn cursors(step: &HeapStep, root_id: &str, node_ids: &HashSet<String>) -> Vec<VizCursor> {
    let mut out = index_cursors(step, root_id, node_ids);
    out.extend(ref_cursors(step, node_ids));
    out
}

fn index_cursors(step: &HeapStep, root_id: &str, node_ids: &HashSet<String>) -> Vec<VizCursor> {
    let Some(HeapObject::Arr { items, .. }) = step.heap.get(root_id) else {
        return Vec::new();
    };
    let len = i64::try_from(items.len()).unwrap_or(i64::MAX);
    rooting::head_locals(step)
        .iter()
        .filter_map(|(name, v)| match v {
            HeapValue::Scalar(HeapScalar::I(i))
                if vocab::is_index_name(name)
                    && *i >= 0
                    && *i < len
                    && node_ids.contains(&format!("{root_id}#{i}")) =>
            {
                Some(VizCursor {
                    name: name.clone(),
                    target: NodeId::new(format!("{root_id}#{i}")),
                    color: String::new(),
                })
            }
            _ => None,
        })
        .collect()
}

fn ref_cursors(step: &HeapStep, node_ids: &HashSet<String>) -> Vec<VizCursor> {
    rooting::head_locals(step)
        .iter()
        .filter_map(|(name, v)| match v {
            HeapValue::Ref(id) if node_ids.contains(id) => Some(VizCursor {
                name: name.clone(),
                target: NodeId::new(id),
                color: String::new(),
            }),
            _ => None,
        })
        .collect()
}
