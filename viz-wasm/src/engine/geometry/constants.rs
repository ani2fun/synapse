//! The one place layout constants live. Pure numbers —
//! shared, cross-compiled, no DOM.

// Graph nodes (trees/graphs)
pub const NODE_R: f64 = 22.0;
pub const RING_R: f64 = 26.0;

// Cell rows (array/stack/queue/bitset/… — the Cells family)
pub const CELL_W: f64 = 46.0;
pub const CELL_H: f64 = 40.0;
pub const CELL_GAP: f64 = 8.0;
/// Horizontal stride between cell columns.
pub const CELL_DX: f64 = CELL_W + CELL_GAP;

// Room reserved above cells for pointer carets, below for index labels
pub const CARET_ROW_H: f64 = 28.0;
pub const INDEX_ROW_H: f64 = 22.0;

// Trees: column stride, row (depth) stride
pub const TREE_COL_W: f64 = 62.0;
pub const TREE_ROW_H: f64 = 82.0;

// Linked lists: horizontal stride between chained nodes (node box + the next-arrow gap)
pub const CHAIN_DX: f64 = 96.0;

// Cursor labels drawn above a node: per-line rise when several stack, and glyph headroom
pub const CURSOR_LINE_H: f64 = 15.0;
pub const CURSOR_GLYPH_UP: f64 = 14.0;

/// Outer padding around a laid-out figure.
pub const PAD: f64 = 12.0;

// Animation durations (milliseconds) — applied as CSS transitions in the renderers
pub const MOVE_MS: u32 = 450;
pub const FADE_MS: u32 = 350;

// Placed linked lists (`layoutHint: "placed"`): the value compartment, one pointer field
// (next, and prev when doubly), the gap a next arrow or a `∅` terminator sits in, and the
// stride between rows (room for the pointer names above the row below).
pub const LIST_VAL_W: f64 = 40.0;
pub const LIST_FIELD_W: f64 = 18.0;
pub const LIST_BOX_H: f64 = 34.0;
pub const LIST_GAP: f64 = 38.0;
pub const LIST_DY: f64 = 100.0;
/// How far a skip arrow arcs above the row, and a back arrow (a cycle) below it.
pub const LIST_ARC: f64 = 30.0;
