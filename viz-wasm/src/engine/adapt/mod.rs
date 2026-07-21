//! The adapter orchestrator: trace → `VizCases` (oracle: `HeapToGraph.scala`, ADR-S030).
//! The ONLY place that knows the stage order — each stage is a pure function in its own
//! module; the named intermediate types make most misorderings uncompilable. Cortex's
//! 1,126-line `HeapToGraph` collapsed all of this into one nested expression; this is the
//! same behaviour, staged and typed. `callstack` is a separate route (no heap root to hunt):
//! cleanup → callstack projection → coalesce → diff → finish — deliberately NO
//! trim/carry-forward (the oracle's asymmetry).

pub mod callstack;
pub mod cards;
pub mod cleanup;
pub mod cursors;
pub mod diff;
pub mod error;
pub mod flow;
pub mod narration;
pub mod projection;
pub mod rooting;
pub mod segmentation;
pub mod snapshot;
pub mod vocab;

pub use error::VizError;

use crate::engine::graph::{VizCases, VizGraph};
use crate::engine::trace::HeapTrace;

/// Adapt a raw heap trace to the render contract. `layout_hint` is the authored `viz=`
/// structure token (or a legacy layout-kind name); `root_hint` names the root variable;
/// `viz_case` caps the detected case count; `title`/`source` decorate the output. Faithful to
/// the oracle's reduction (quirk, ported on purpose): any surviving segment wins; the error
/// surfaces only if EVERY segment failed — and then it's the FIRST error.
pub fn adapt(
    trace: &HeapTrace,
    source: &str,
    layout_hint: &str,
    root_hint: Option<&str>,
    viz_case: Option<u32>,
    title: &str,
) -> Result<VizCases, VizError> {
    if trace.steps.is_empty() {
        return Err(VizError::EmptyTrace);
    }
    let cleaned = cleanup::clean(trace, root_hint);
    if cleaned.steps.is_empty() {
        return Err(VizError::OnlyBuilderFrames);
    }
    if layout_hint == "callstack" {
        return adapt_callstack(&cleaned, source, title).map(|g| VizCases { cases: vec![g] });
    }
    let segments = segmentation::segment(&cleaned, root_hint, viz_case);
    let results: Vec<Result<VizGraph, VizError>> = segments
        .into_iter()
        .map(|seg| adapt_segment(seg, source, layout_hint, root_hint, cleaned.truncated, title))
        .collect();
    let graphs: Vec<VizGraph> = results.iter().filter_map(|r| r.as_ref().ok().cloned()).collect();
    if graphs.is_empty() {
        Err(results
            .into_iter()
            .find_map(Result::err)
            .unwrap_or(VizError::EmptyTrace))
    } else {
        Ok(VizCases { cases: graphs })
    }
}

// ── one segment (one test case) → one VizGraph ──
fn adapt_segment(
    seg: segmentation::TraceSegment,
    source: &str,
    layout_hint: &str,
    root_hint: Option<&str>,
    truncated: bool,
    title: &str,
) -> Result<VizGraph, VizError> {
    let rooted = rooting::resolve(seg.steps, root_hint, layout_hint);
    if rooted.root_id.is_none() {
        return Err(VizError::NoRoot);
    }
    let projected = projection::project(&rooted, root_hint, layout_hint);
    let flowed = flow::trim_and_fill(projected);
    let diffed = diff::diff(&flowed)?;
    let graph = narration::finish(&diffed, source, layout_hint, title, truncated);
    if graph.steps.iter().all(|s| s.nodes.is_empty()) {
        Err(VizError::RootNeverHeldStructure)
    } else {
        Ok(graph)
    }
}

// ── the call-stack route ──
fn adapt_callstack(cleaned: &cleanup::CleanedTrace, source: &str, title: &str) -> Result<VizGraph, VizError> {
    let projected = callstack::project(cleaned);
    let coalesced = flow::coalesce(projected);
    let diffed = diff::diff(&coalesced)?;
    let graph = narration::finish(&diffed, source, "callstack", title, cleaned.truncated);
    if graph.steps.iter().all(|s| s.nodes.is_empty()) {
        Err(VizError::NoCallFrames)
    } else {
        Ok(graph)
    }
}
