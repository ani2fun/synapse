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
