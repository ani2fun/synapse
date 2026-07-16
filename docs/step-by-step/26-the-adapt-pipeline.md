# Step 26 — The adapt pipeline: trace → VizCases, proven against the cortex-goldens

*(oracle: synapse steps 28–29's shared half — `HeapTrace` + the whole `viz/adapt` staged
pipeline, ADR-S029/S030 — the heart of RS-P7.)*

## The staged pipeline (`shared/src/viz/adapt/`)

`adapt()` is the ONLY place that knows the stage order; each stage is a pure function in its
own module and the named intermediate types make most misorderings uncompilable:
`HeapTrace → cleanup (string synthesis + helper-frame filter) → segmentation (one graph per
test case; a rebind splits only when DISCONNECTED both directions — rotations and recursive
descents don't) → rooting (dotted → local → attr → auto-detect, sorted deterministic
tiebreaks; the per-step rotation guard, ADR-0027) → projection (reachable-minus-sentinels;
instances/arrays/dicts to nodes+edges; union-find cards; per-card layout inference with the
authored override on the ROOT card only) → flow (drop-empty-ends keeping the last leading
blank · carry-forward · coalesce) → diff (highlight/changed/removed re-emitted once; the
newly-appeared-local quirk ported faithfully; `unchanged` includes EDGES — delta #8;
duplicate node id = loud `VizError`, never a silent dedup) → narration (caption precedence:
removed → insert/added → changed → cursor-moved → initial → source line; colours assigned
ONCE, LAST)`. `callstack` is its own route with the oracle's deliberate asymmetry (no
trim/carry-forward). `HeapStep.heap` is a `BTreeMap` — every scan deterministic by
construction (the JVM↔JS map-order hazard, solved structurally in Rust).

## The parity gate

The 16 cortex-goldens (`shared/tests/fixtures/cortex-goldens/`) are Cortex's own finished
adapter output, copied verbatim. The paired INPUTS — the oracle's ~800 LOC of hand-built
Scala traces — were not hand-ported: a throwaway (never-committed) exporter ran inside the
oracle and serialised `CortexFixtures.all` to JSON in serde's exact shapes, so the fixture
data is the oracle's bit-for-bit. The harness adapts each input through the REAL Rust
pipeline and compares canonical JSON after `VizParity.normalize` (erasing exactly the three
deliberate-delta fields: `structureType`, `cardCursor`, `unchanged` — each an ADR-S030 row);
any other difference fails with a per-step field diagnosis.

**All 16 goldens matched on the first run** — array · avl-rotation · bitset · fenwick ·
graph-bfs · graph-kind · hashmap-chained-collisions · hashmap-kind · heap · linked-list ·
queue · segment-tree · skiplist · stack-push · trie · union-find. The Rust pipeline is
parity-exact with the Scala oracle (which was parity-exact with Cortex).

## Tests

+17: the golden gate (16 fixtures through one loud test) and 16 stage tests pinning what the
goldens can't isolate — string synthesis + the Java-builder uppercase gate, preorder
memoized reachability, the CLRS sentinel, the rooting ladder + dotted paths, both
segmentation directions, the three flow passes, the diff cue calculus + re-emit-once + the
loud duplicate id, the typed error surface, the callstack route, and narration precedence
(including the honest finding that a step-0 cursor narrates its placement — the goldens
agree). Suite: 329 Rust + 40 vitest; compiles clean for `wasm32`.

Next: the widget spine — the client host + renderer families that draw these `VizCases`.
