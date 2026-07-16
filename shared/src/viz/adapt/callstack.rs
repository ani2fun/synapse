//! The `viz=callstack` route (oracle: `CallStackProjection.scala`): each non-helper frame
//! becomes a slot node in a growing stack (innermost frame on top), labelled
//! `fn(firstIntArg)`. No segmentation, no heap projection — the call stack IS the structure.

use crate::viz::adapt::cleanup::CleanedTrace;
use crate::viz::adapt::projection::{self, ProjectedNode, ProjectedStep};
use crate::viz::adapt::vocab;
use crate::viz::graph::{NodeId, VizNode};
use crate::viz::trace::{HeapFrame, HeapScalar, HeapStep, HeapValue};

#[must_use]
pub fn project(cleaned: &CleanedTrace) -> Vec<ProjectedStep> {
    cleaned.steps.iter().map(build_step).collect()
}

fn build_step(step: &HeapStep) -> ProjectedStep {
    let frames: Vec<&HeapFrame> = step
        .frames
        .iter()
        .filter(|f| !vocab::is_helper_frame(&f.fn_name))
        .collect();
    let depth = frames.len();
    let nodes: Vec<ProjectedNode> = frames
        .iter()
        .enumerate()
        .map(|(i, frame)| {
            // Outermost frame at the bottom (slot 0), active frame on top.
            let slot = depth - 1 - i;
            let id = format!("__syn_frame_{slot}");
            ProjectedNode {
                node: VizNode {
                    id: NodeId::new(&id),
                    label: frame_stack_label(frame),
                    kind: "frame".to_owned(),
                    slot: Some(i32::try_from(slot).unwrap_or(i32::MAX)),
                    ..VizNode::default()
                },
                owner: id,
            }
        })
        .collect();
    ProjectedStep {
        line: step.line,
        event: step.event.clone(),
        nodes,
        edges: Vec::new(),
        cursor: Vec::new(),
        frames: projection::build_frames(step),
        structure_type: Some("stack".to_owned()),
    }
}

fn frame_stack_label(frame: &HeapFrame) -> String {
    if frame.fn_name == "main" || frame.fn_name == "<module>" {
        frame.fn_name.clone()
    } else {
        frame
            .locals
            .iter()
            .find_map(|(_, v)| match v {
                HeapValue::Scalar(HeapScalar::I(i)) => Some(*i),
                _ => None,
            })
            .map_or_else(|| frame.fn_name.clone(), |v| format!("{}({v})", frame.fn_name))
    }
}
