//! 2-D cell grid: cells placed by `slot` into (row, col), the
//! column count derived from the cell count (√n). Traced grid runs refine it once the
//! adapter lands.

use std::collections::HashMap;

use super::constants::{CELL_DX, CELL_GAP, CELL_H, PAD};
use super::{LayoutResult, Point};
use crate::engine::graph::{VizEdge, VizNode};

#[must_use]
pub fn grid(nodes: &[VizNode], _edges: &[VizEdge]) -> LayoutResult {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let cols = ((nodes.len() as f64).sqrt().round() as i32).max(1);
    let row_h = CELL_H + CELL_GAP;
    let slot_of = |node: &VizNode, i: usize| -> i32 {
        node.slot.unwrap_or_else(|| i32::try_from(i).unwrap_or(i32::MAX))
    };
    let positions: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let slot = slot_of(node, i);
            let r = slot / cols;
            let c = slot % cols;
            (
                node.id.clone(),
                Point {
                    x: PAD + f64::from(c) * CELL_DX,
                    y: PAD + f64::from(r) * row_h,
                },
            )
        })
        .collect();
    let max_slot = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| slot_of(node, i))
        .max()
        .unwrap_or(0);
    let rows = max_slot / cols + 1;
    LayoutResult {
        positions,
        width: PAD * 2.0 + f64::from(cols) * CELL_DX - CELL_GAP,
        height: PAD * 2.0 + f64::from(rows) * row_h - CELL_GAP,
    }
}
