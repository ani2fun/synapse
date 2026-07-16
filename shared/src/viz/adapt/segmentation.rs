//! Stage 2 (oracle: `TraceSegmentation.scala`): one `VizGraph` per test case. A new case
//! begins when the root variable rebinds to a structure DISCONNECTED from the current case's
//! root, read from the LIVE heap BOTH directions — `new→old` covers a rotation lifting a node
//! above the root, `old→new` covers a recursive descent into a child; neither starts a new
//! case. `viz-case=N` keeps exactly N−1 split points. No root hint → one case.

use crate::viz::adapt::cleanup::CleanedTrace;
use crate::viz::adapt::rooting;
use crate::viz::adapt::snapshot::HeapSnapshot;
use crate::viz::trace::HeapStep;

/// One test case's contiguous steps.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceSegment {
    pub steps: Vec<HeapStep>,
}

#[must_use]
pub fn segment(cleaned: &CleanedTrace, root_hint: Option<&str>, viz_case: Option<u32>) -> Vec<TraceSegment> {
    let Some(hint) = root_hint else {
        return vec![TraceSegment {
            steps: cleaned.steps.clone(),
        }];
    };
    let boundaries = case_boundaries(&cleaned.steps, hint);
    let splits: Vec<usize> = match viz_case {
        Some(n) if n >= 1 => boundaries.iter().skip(1).take(n as usize - 1).copied().collect(),
        _ => boundaries.iter().skip(1).copied().collect(),
    };
    slice_at(&cleaned.steps, &splits)
}

fn case_boundaries(steps: &[HeapStep], hint: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut case_root: Option<String> = None;
    for (i, step) in steps.iter().enumerate() {
        if let Some(rid) = rooting::root_id_in_step(step, hint) {
            let snap = HeapSnapshot::new(&step.heap);
            let connected = case_root.as_ref().is_some_and(|cr| {
                rid == *cr
                    || snap.reachable_from(&rid).iter().any(|r| r == cr)
                    || snap.reachable_from(cr).contains(&rid)
            });
            if !connected {
                out.push(i);
                case_root = Some(rid);
            }
        }
    }
    out
}

fn slice_at(steps: &[HeapStep], splits: &[usize]) -> Vec<TraceSegment> {
    let mut bounds = vec![0usize];
    bounds.extend_from_slice(splits);
    bounds.push(steps.len());
    bounds
        .windows(2)
        .filter(|w| w[0] < w[1])
        .map(|w| TraceSegment {
            steps: steps[w[0]..w[1]].to_vec(),
        })
        .collect()
}
