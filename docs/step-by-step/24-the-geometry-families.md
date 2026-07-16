# Step 24 — The geometry families: layout-once, and a force sim that never wobbles

*(oracle: synapse step 26 part 2 + step 27 whole — `Geometry`, `Layout`, and the five layout
families. Pure math: no DOM, no IO, natively tested.)*

## The invariant and the constants

`geometry::union(graph)` collects every step's nodes (deduped by id, FIRST occurrence wins)
and edges — and a layout is computed ONCE over that union (the oracle's stability invariant,
ADR-0018): a node's position never shifts between steps; per-step rendering only toggles
presence and diff classes. `constants` is the one place layout numbers live (`NODE_R` 22,
`CELL_W/H` 46/40, `TREE_COL_W/ROW_H` 62/82, `CHAIN_DX` 96, caret/index rows, `PAD` 12,
`MOVE_MS`/`FADE_MS`) — Cortex copy-pasted `NODE_R=22` across ~11 files; here a change lands
once.

## The families

- **linear** — the Cells row (each node in its `slot` column, gaps stay empty, caret row
  above / index row below) and the vertical LIFO **stack** (slot 0 at the bottom, a row
  reserved for the ↑ TOP marker).
- **grid** — 2-D cells by `slot` into (row, col), columns ≈ √n.
- **tree** — the recursive subtree-width walk: a leaf takes the next column, an internal
  node centres over its children, depth is the row; children order by edge label
  (left < right < other) so a BST reads left-to-right and a skewed chain cascades straight
  down; a visited guard means cycles can't hang; disconnected nodes get fresh columns.
- **chain** — `next`-chain order from the head (the node with no incoming `next`); `prev`
  edges ignored for placement; a cycle or merge sets `graph_fallback` so pathological lists
  can route to the graph canvas.
- **graph_layout** — DUAL: an acyclic ≤1-parent union lays out as a tidy forest (multiple
  roots hang under a synthetic `__syn_superroot` that is then dropped and the rows lifted);
  anything cyclic falls to the SEEDED force sim.

## The deterministic force sim

Velocity-Verlet with the oracle's constants verbatim — link 100 · manyBody −520 (naive
O(n²)) · collide `RING_R`+12 · x/y anchor 0.07 · velocityDecay 0.6 · 320 synchronous ticks —
seeded by a ported **Mulberry32(0x5eed)** (u32 wrapping arithmetic, bit-identical to the
Scala/JS original) for the jiggle, d3's phyllotaxis init, and ordered Vec iteration
throughout (never map order — the JVM↔JS lesson carried into Rust, where `HashMap` order is
just as unspecified). The arithmetic ORDER is part of the determinism contract, so the sim
stays one straight-line function. **A redraw is byte-identical** — pinned by tests, twice
(plain and with a disconnected node). Pixel-parity with d3 is a NON-goal (ADR-S026);
determinism + readability is the contract.

## Tests

+47, the oracle suites case-for-case: array 8 + union 2 (incl. the
lays-out-cells-that-appear-only-later pin) · stack 7 · tree 10 (root on top, per-level rows,
centred parents, strictly increasing in-order columns, the cycle-does-not-hang guard) ·
chain 8 (head found regardless of input order, prev ignored, cycle/merge flags) · graph 12
(byte-identical redraws, forest-vs-force dispatch, the dropped super-root, positive padded
positions). Suite: 312 Rust + 40 vitest. Everything compiles for `wasm32` — the client
renderers consume these functions in the widget step.

Next: **the design system + dark mode** — the token port the upcoming renderers need
(`--viz-role-*` maps the wire hexes to theme-aware colours), and the visual parity the RS
client has been deferring: fonts, the theme toggle + pre-paint bootstrap, and the existing
pages restyled onto the tokens.
