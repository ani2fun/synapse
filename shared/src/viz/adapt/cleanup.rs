//! Stage 1 (oracle: `TraceCleanup.scala`): string synthesis + helper-step filter. If the
//! root hint names a string local, materialise it as an `Arr` of 1-char cells so two-pointer
//! string problems render as an array; then drop steps whose active frame is a constructor/
//! builder (setup noise, not algorithm steps).

use crate::viz::adapt::vocab;
use crate::viz::trace::{ArrKind, HeapObject, HeapScalar, HeapStep, HeapTrace, HeapValue};

/// The trace after cleanup.
#[derive(Debug, Clone, PartialEq)]
pub struct CleanedTrace {
    pub steps: Vec<HeapStep>,
    pub truncated: bool,
}

#[must_use]
pub fn clean(trace: &HeapTrace, root_hint: Option<&str>) -> CleanedTrace {
    let synthesised = synthesize_string_arrays(trace, root_hint);
    let kept = synthesised
        .steps
        .into_iter()
        .filter(|step| {
            !step
                .frames
                .first()
                .is_some_and(|f| vocab::is_helper_frame(&f.fn_name))
        })
        .collect();
    CleanedTrace {
        steps: kept,
        truncated: trace.truncated,
    }
}

// `viz-root=s` naming a string local → an `Arr` of 1-char cells at a stable synthetic id,
// the local rebound to a Ref to it.
fn synthesize_string_arrays(trace: &HeapTrace, root_hint: Option<&str>) -> HeapTrace {
    let Some(name) = root_hint.filter(|n| !n.contains('.')) else {
        return trace.clone();
    };
    let synth_id = format!("__syn_str_{name}");
    let steps = trace
        .steps
        .iter()
        .map(|step| {
            let maybe_str = step.frames.first().and_then(|f| {
                f.locals.iter().find_map(|(n, v)| match v {
                    HeapValue::Scalar(HeapScalar::S(s)) if n == name => Some(s.clone()),
                    _ => None,
                })
            });
            let Some(s) = maybe_str else {
                return step.clone();
            };
            let items = s
                .chars()
                .map(|c| HeapValue::Scalar(HeapScalar::S(c.to_string())))
                .collect();
            let mut new_heap = step.heap.clone();
            new_heap.insert(
                synth_id.clone(),
                HeapObject::Arr {
                    kind: ArrKind::Lst,
                    items,
                },
            );
            let mut new_frames = step.frames.clone();
            if let Some(head) = new_frames.first_mut() {
                for (n, v) in &mut head.locals {
                    if n == name {
                        *v = HeapValue::Ref(synth_id.clone());
                    }
                }
            }
            HeapStep {
                heap: new_heap,
                frames: new_frames,
                ..step.clone()
            }
        })
        .collect();
    HeapTrace {
        steps,
        truncated: trace.truncated,
    }
}
