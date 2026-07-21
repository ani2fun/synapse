//! Stage 5 (oracle: `StepFlow.scala`): trim · fill · coalesce, oracle order. (1)
//! `drop_empty_ends` trims blank steps off both ends, KEEPING exactly the last leading blank
//! (the frame that opens the animation); (2) `carry_forward` fills interior blanks with the
//! previous non-empty graph; (3) `coalesce` drops exact consecutive duplicates on
//! (line, nodes, edges, cursor) — NOT annotation (captions aren't built yet).

use crate::engine::adapt::projection::ProjectedStep;

/// The general-path composition: `drop_empty_ends` → `carry_forward` → `coalesce`.
#[must_use]
pub fn trim_and_fill(steps: Vec<ProjectedStep>) -> Vec<ProjectedStep> {
    coalesce(carry_forward(drop_empty_ends(steps)))
}

/// Trailing blanks all drop; leading blanks drop except the last (the opening frame).
#[must_use]
#[allow(clippy::needless_pass_by_value)] // by-value keeps the three passes one pipeline
pub fn drop_empty_ends(steps: Vec<ProjectedStep>) -> Vec<ProjectedStep> {
    let last_non_empty = steps.iter().rposition(|s| !s.nodes.is_empty());
    let Some(end) = last_non_empty else {
        return Vec::new(); // all blank → the span keeps nothing (leading has no last before rest)
    };
    let no_trailing = &steps[..=end];
    let first_non_empty = no_trailing
        .iter()
        .position(|s| !s.nodes.is_empty())
        .unwrap_or(no_trailing.len());
    let keep_from = first_non_empty.saturating_sub(1);
    no_trailing[keep_from..].to_vec()
}

/// Interior blanks inherit the previous non-empty step's nodes+edges (keeping their own
/// line); the leading blank stays blank. Length-preserving.
#[must_use]
pub fn carry_forward(steps: Vec<ProjectedStep>) -> Vec<ProjectedStep> {
    let mut out: Vec<ProjectedStep> = Vec::with_capacity(steps.len());
    let mut last: Option<ProjectedStep> = None;
    for s in steps {
        if s.nodes.is_empty() {
            match &last {
                Some(prev) => out.push(ProjectedStep {
                    nodes: prev.nodes.clone(),
                    edges: prev.edges.clone(),
                    ..s
                }),
                None => out.push(s),
            }
        } else {
            out.push(s.clone());
            last = Some(s);
        }
    }
    out
}

/// Drop a step equal to its predecessor on (line, nodes, edges, cursor). Idempotent.
#[must_use]
pub fn coalesce(steps: Vec<ProjectedStep>) -> Vec<ProjectedStep> {
    let mut out: Vec<ProjectedStep> = Vec::with_capacity(steps.len());
    for s in steps {
        let differs = out.last().is_none_or(|prev| {
            s.line != prev.line || s.nodes != prev.nodes || s.edges != prev.edges || s.cursor != prev.cursor
        });
        if differs {
            out.push(s);
        }
    }
    out
}
