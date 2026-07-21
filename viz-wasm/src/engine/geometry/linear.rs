//! The Cells family: a contiguous row of cells, one column
//! per `slot`; and the vertical LIFO stack. Room is reserved above for pointer carets and
//! below for index labels; the client renderer turns positions into SVG.

use std::collections::HashMap;

use super::constants::{CARET_ROW_H, CELL_DX, CELL_GAP, CELL_H, CELL_W, INDEX_ROW_H, PAD};
use super::{LayoutResult, Point};
use crate::engine::graph::{VizEdge, VizNode};

/// An array/cell row: each node sits in its `slot` column (slot-less nodes fall to column 0).
/// Computed over the union of all steps, so a cell keeps its column whether or not it's
/// present in a given step.
#[must_use]
pub fn array(nodes: &[VizNode], _edges: &[VizEdge]) -> LayoutResult {
    let max_slot = nodes.iter().filter_map(|n| n.slot).max().unwrap_or(0);
    let positions: HashMap<_, _> = nodes
        .iter()
        .map(|n| {
            let col = f64::from(n.slot.unwrap_or(0));
            (
                n.id.clone(),
                Point {
                    x: PAD + col * CELL_DX,
                    y: CARET_ROW_H,
                },
            )
        })
        .collect();
    LayoutResult {
        positions,
        width: PAD * 2.0 + f64::from(max_slot + 1) * CELL_DX - CELL_GAP,
        height: CARET_ROW_H + CELL_H + INDEX_ROW_H,
    }
}

/// A vertical LIFO stack: slot 0 at the bottom, higher slots stacked upward, with a row
/// reserved on top for the "↑ TOP" marker. `x` is fixed; `y` decreases as the slot grows.
#[must_use]
pub fn stack(nodes: &[VizNode], _edges: &[VizEdge]) -> LayoutResult {
    let max_slot = nodes.iter().filter_map(|n| n.slot).max().unwrap_or(0);
    let stride = CELL_H + CELL_GAP;
    let top_row = CARET_ROW_H; // room for the ↑ TOP marker
    let positions: HashMap<_, _> = nodes
        .iter()
        .map(|n| {
            let s = n.slot.unwrap_or(0);
            (
                n.id.clone(),
                Point {
                    x: PAD,
                    y: PAD + top_row + f64::from(max_slot - s) * stride,
                },
            )
        })
        .collect();
    LayoutResult {
        positions,
        width: PAD * 2.0 + CELL_W + 44.0, // room for index labels on the right
        height: PAD * 2.0 + top_row + f64::from(max_slot + 1) * stride - CELL_GAP,
    }
}

#[cfg(test)]
#[path = "linear_tests.rs"]
mod tests;
