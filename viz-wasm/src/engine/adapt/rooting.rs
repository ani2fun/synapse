//! Stage 3: which heap object is the structure's root.
//! Deliberately two-pass (NOT one orElse chain — the pass structure is semantic): the hint
//! resolves as a dotted path, else a local, else an instance attribute; failing all,
//! auto-detect. Plus the shared per-step queries and `step_root_for` — the per-step
//! re-resolution with the one-directional guard that keeps a segment together through
//! rotations.

use std::collections::BTreeMap;

use crate::engine::adapt::snapshot::HeapSnapshot;
use crate::engine::trace::{HeapObject, HeapStep, HeapValue};

/// A segment plus its resolved root id (`None` → the segment draws empty).
#[derive(Debug, Clone, PartialEq)]
pub struct RootedSegment {
    pub steps: Vec<HeapStep>,
    pub root_id: Option<String>,
}

pub fn head_locals(step: &HeapStep) -> &[(String, HeapValue)] {
    step.frames
        .first()
        .map(|f| f.locals.as_slice())
        .unwrap_or_default()
}

#[must_use]
pub fn resolve(steps: Vec<HeapStep>, root_hint: Option<&str>, layout_hint: &str) -> RootedSegment {
    let root_id = resolve_root_id(&steps, root_hint, layout_hint);
    RootedSegment { steps, root_id }
}

// ── segment-level resolution: a NAMED root, dotted → local → attr; else auto-detect ──
/// A root someone NAMED is a claim about the program, and the only two honest answers to it are
/// the object it names and none at all. Falling through to auto-detect draws a DIFFERENT object
/// under the name they typed — the reader asked "show me `arr`" and is shown something else,
/// labelled convincingly, with nothing anywhere saying a substitution happened. That is the one
/// failure they cannot see, and it is worth more than the runs the fallback rescued.
///
/// Auto-detect keeps the job it was written for: nobody named anything, so anything reachable is
/// a fair guess and a wrong guess costs only a re-pick.
#[must_use]
pub fn resolve_root_id(steps: &[HeapStep], root_hint: Option<&str>, layout_hint: &str) -> Option<String> {
    match root_hint.map(str::trim).filter(|hint| !hint.is_empty()) {
        Some(hint) if hint.contains('.') => resolve_dotted(steps, hint),
        Some(hint) => resolve_local(steps, hint).or_else(|| resolve_attr(steps, hint)),
        None => auto_detect_root(steps, layout_hint),
    }
}

fn resolve_local(steps: &[HeapStep], name: &str) -> Option<String> {
    steps.iter().flat_map(head_locals).find_map(|(n, v)| match v {
        HeapValue::Ref(id) if n == name => Some(id.clone()),
        _ => None,
    })
}

fn resolve_attr(steps: &[HeapStep], name: &str) -> Option<String> {
    steps
        .iter()
        .flat_map(|s| s.heap.values())
        .filter_map(|o| match o {
            HeapObject::Instance { fields, .. } => Some(fields),
            _ => None,
        })
        .find_map(|fields| {
            fields.iter().find_map(|(f, v)| match v {
                HeapValue::Ref(id) if f == name => Some(id.clone()),
                _ => None,
            })
        })
}

fn resolve_dotted(steps: &[HeapStep], path: &str) -> Option<String> {
    let mut parts = path.split('.');
    let head = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    steps.iter().find_map(|step| {
        head_locals(step)
            .iter()
            .find_map(|(n, v)| match v {
                HeapValue::Ref(id) if n == head => Some(id.clone()),
                _ => None,
            })
            .and_then(|id| follow_fields(&id, &rest, &step.heap))
    })
}

fn follow_fields(id: &str, fields: &[&str], heap: &BTreeMap<String, HeapObject>) -> Option<String> {
    let Some((f, rest)) = fields.split_first() else {
        return Some(id.to_owned());
    };
    match heap.get(id) {
        Some(HeapObject::Instance { fields: fs, .. }) => fs
            .iter()
            .find_map(|(n, v)| match v {
                HeapValue::Ref(to) if n == f => Some(to.clone()),
                _ => None,
            })
            .and_then(|to| follow_fields(&to, rest, heap)),
        _ => None,
    }
}

fn auto_detect_root(steps: &[HeapStep], layout_hint: &str) -> Option<String> {
    let array_root = if layout_hint.to_lowercase().contains("array") {
        steps.iter().find_map(|s| {
            head_locals(s).iter().find_map(|(_, v)| match v {
                HeapValue::Ref(id) if matches!(s.heap.get(id), Some(HeapObject::Arr { .. })) => {
                    Some(id.clone())
                }
                _ => None,
            })
        })
    } else {
        None
    };
    array_root.or_else(|| {
        let heap = steps.last().map(|s| &s.heap)?;
        if heap.is_empty() {
            return None;
        }
        let snap = HeapSnapshot::new(heap);
        let referenced: std::collections::HashSet<&str> =
            heap.values().flat_map(HeapSnapshot::out_refs).collect();
        // Functions and classes are memory, not data: the reader's `Solution` is unreferenced and
        // reaches every method it has, which on a small structure would make it the biggest thing
        // on the heap — and the structure drawn would be a class.
        let data: Vec<&String> = heap
            .iter()
            .filter(|(_, object)| !object.is_code())
            .map(|(id, _)| id)
            .collect();
        let roots: Vec<&String> = data
            .iter()
            .copied()
            .filter(|id| !referenced.contains(id.as_str()))
            .collect();
        let pool: Vec<&String> = if roots.is_empty() { data } else { roots };
        // Sorted first so ties break deterministically, then max by reachable size — this
        // keeps the FIRST maximum, i.e. the smallest id among ties.
        let mut sorted = pool;
        sorted.sort();
        let mut best: Option<(&String, usize)> = None;
        for id in sorted {
            let size = snap.reachable_from(id).len();
            if best.is_none_or(|(_, s)| size > s) {
                best = Some((id, size));
            }
        }
        best.map(|(id, _)| id.clone())
    })
}

// ── per-step queries ──
/// The root id in a single step (segmentation boundaries + per-step re-resolution).
#[must_use]
pub fn root_id_in_step(step: &HeapStep, hint: &str) -> Option<String> {
    if hint.contains('.') {
        let mut parts = hint.split('.');
        let head = parts.next()?;
        let rest: Vec<&str> = parts.collect();
        head_locals(step)
            .iter()
            .find_map(|(n, v)| match v {
                HeapValue::Ref(id) if n == head => Some(id.clone()),
                _ => None,
            })
            .and_then(|id| follow_fields(&id, &rest, &step.heap))
    } else {
        head_locals(step)
            .iter()
            .find_map(|(n, v)| match v {
                HeapValue::Ref(id) if n == hint => Some(id.clone()),
                _ => None,
            })
            .or_else(|| {
                step.heap.values().find_map(|o| match o {
                    HeapObject::Instance { fields, .. } => fields.iter().find_map(|(f, v)| match v {
                        HeapValue::Ref(id) if f == hint => Some(id.clone()),
                        _ => None,
                    }),
                    _ => None,
                })
            })
    }
}

/// Re-resolve the root for one step, adopting a re-bound target only if it still reaches the
/// segment's original root (a rotation lifts a sibling above the root and still reaches it; a
/// recursive descent doesn't reach back up, so the guard keeps the segment root).
#[must_use]
pub fn step_root_for(step: &HeapStep, root_hint: Option<&str>, root_id: &str) -> String {
    root_hint
        .and_then(|h| root_id_in_step(step, h))
        .filter(|t| {
            HeapSnapshot::new(&step.heap)
                .reachable_from(t)
                .iter()
                .any(|r| r == root_id)
        })
        .unwrap_or_else(|| root_id.to_owned())
}
