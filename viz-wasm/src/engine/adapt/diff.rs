//! Stage 6: per-step highlight / changed / removed + `unchanged`.
//! Compare each step to its PRE-diff predecessor (a re-emitted removed node fades exactly
//! once). By node id: highlight = new ids; changed = ids whose LABEL differs; removed = ids
//! gone (re-emitted carrying their last label). `unchanged` DELIBERATELY includes edges, so
//! a rotation doesn't hide behind diff mode. Two nodes sharing an id is a loud error, never a
//! silent map dedup.

use std::collections::{HashMap, HashSet};

use crate::engine::adapt::error::VizError;
use crate::engine::adapt::projection::ProjectedStep;
use crate::engine::graph::{NodeId, VizCursor, VizEdge, VizFrame, VizNode};

/// One step after diffing — pre-narration (the caption is added next).
#[derive(Debug, Clone, PartialEq)]
pub struct DiffedStep {
    pub line: i32,
    pub event: String,
    pub nodes: Vec<VizNode>,
    pub edges: Vec<VizEdge>,
    pub cursor: Vec<VizCursor>,
    pub highlight: Vec<NodeId>,
    pub changed: Vec<NodeId>,
    pub removed: Vec<NodeId>,
    pub frames: Vec<VizFrame>,
    pub unchanged: bool,
    pub structure_type: Option<String>,
}

pub fn diff(steps: &[ProjectedStep]) -> Result<Vec<DiffedStep>, VizError> {
    if let Some(id) = first_duplicate_id(steps) {
        return Err(VizError::DuplicateNodeId(id));
    }
    Ok((0..steps.len()).map(|i| diff_at(steps, i)).collect())
}

fn first_duplicate_id(steps: &[ProjectedStep]) -> Option<String> {
    steps.iter().find_map(|s| {
        let mut seen = HashSet::new();
        s.nodes
            .iter()
            .map(|pn| pn.node.id.value())
            .find(|id| !seen.insert(id.to_owned()))
            .map(str::to_owned)
    })
}

// One straight pass per step — splitting the cue calculus would hide the compare set.
#[allow(clippy::too_many_lines)]
fn diff_at(steps: &[ProjectedStep], i: usize) -> DiffedStep {
    let cur = &steps[i];
    let cur_nodes: Vec<VizNode> = cur.nodes.iter().map(|pn| pn.node.clone()).collect();
    if i == 0 {
        // Step 1 gets no diff cues (narrate(None) → "initial structure").
        return DiffedStep {
            line: cur.line,
            event: cur.event.clone(),
            nodes: cur_nodes,
            edges: cur.edges.clone(),
            cursor: cur.cursor.clone(),
            highlight: Vec::new(),
            changed: Vec::new(),
            removed: Vec::new(),
            frames: cur.frames.clone(),
            unchanged: false,
            structure_type: cur.structure_type.clone(),
        };
    }
    let prev = &steps[i - 1];
    let prev_nodes: Vec<VizNode> = prev.nodes.iter().map(|pn| pn.node.clone()).collect();
    let prev_labels: HashMap<&NodeId, &str> = prev_nodes.iter().map(|n| (&n.id, n.label.as_str())).collect();
    let cur_labels: HashMap<&NodeId, &str> = cur_nodes.iter().map(|n| (&n.id, n.label.as_str())).collect();
    let highlight: Vec<NodeId> = cur_nodes
        .iter()
        .map(|n| n.id.clone())
        .filter(|id| !prev_labels.contains_key(id))
        .collect();
    let changed: Vec<NodeId> = cur_nodes
        .iter()
        .map(|n| n.id.clone())
        .filter(|id| {
            prev_labels
                .get(id)
                .is_some_and(|p| Some(*p) != cur_labels.get(id).copied())
        })
        .collect();
    let removed: Vec<VizNode> = prev_nodes
        .iter()
        .filter(|n| !cur_labels.contains_key(&n.id))
        .cloned()
        .collect();

    // Per-local changed flag, keyed (fn, name) → prev value. A deliberate quirk: a
    // NEWLY-appeared local stays changed=false.
    let prev_locals: HashMap<(&str, &str), &str> = prev
        .frames
        .iter()
        .flat_map(|f| {
            f.locals
                .iter()
                .map(move |l| ((f.fn_name.as_str(), l.name.as_str()), l.value.as_str()))
        })
        .collect();
    let frames: Vec<VizFrame> = cur
        .frames
        .iter()
        .map(|f| VizFrame {
            fn_name: f.fn_name.clone(),
            locals: f
                .locals
                .iter()
                .map(|l| {
                    let changed = prev_locals
                        .get(&(f.fn_name.as_str(), l.name.as_str()))
                        .is_some_and(|prev_value| *prev_value != l.value);
                    crate::engine::graph::VizLocal { changed, ..l.clone() }
                })
                .collect(),
            is_active: f.is_active,
        })
        .collect();

    // unchanged: nodes (ids+labels), EDGES (delta #8), cursor (name,target), and
    // active-frame locals all equal.
    let cursor_pairs = |s: &ProjectedStep| -> HashSet<(String, String)> {
        s.cursor
            .iter()
            .map(|c| (c.name.clone(), c.target.value().to_owned()))
            .collect()
    };
    let head_locals = |s: &ProjectedStep| -> Vec<(String, String, String)> {
        s.frames
            .first()
            .map(|f| {
                f.locals
                    .iter()
                    .map(|l| (l.name.clone(), l.type_name.clone(), l.value.clone()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let unchanged = highlight.is_empty()
        && changed.is_empty()
        && removed.is_empty()
        && prev.edges == cur.edges
        && cursor_pairs(prev) == cursor_pairs(cur)
        && head_locals(prev) == head_locals(cur);

    let removed_ids: Vec<NodeId> = removed.iter().map(|n| n.id.clone()).collect();
    let mut nodes = cur_nodes;
    nodes.extend(removed);
    DiffedStep {
        line: cur.line,
        event: cur.event.clone(),
        nodes,
        edges: cur.edges.clone(),
        cursor: cur.cursor.clone(),
        highlight,
        changed,
        removed: removed_ids,
        frames,
        unchanged,
        structure_type: cur.structure_type.clone(),
    }
}
