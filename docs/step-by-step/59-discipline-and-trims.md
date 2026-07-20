# Step 59 — Discipline made structural, and the interface trims

*(Five small cuts from the deepening loop's items 3+7 — every one about an interface saying
exactly what it means.)*

## The ask

The `/codebase-design` assessment's item 3 (gate coverage) and item 7 (small trims) — fused
into one step because each piece is mechanical, and together they share a theme: make the
architecture *enforced* where it was merely observed, and make every `pub` mean "someone
outside needs this".

## a · The purity gate never covered the viz engine

The step-58 survey claimed the conventions gate covered `logic/` and `viz/engine/`; the
extraction pass corrected it — gate 2 matched ONLY `*/logic/*`. The whole engine (moved out
of synapse-shared in step 45 precisely for its purity) was unprotected, and `shapes.rs` +
`decoder.rs` — pure engine logic — sat at `viz/` root where no future gate arm could
reasonably reach them without also sweeping the leptos-side glue.

Both moved into `client/src/viz/engine/` (five import sites re-pathed), and gate 2's `find`
grew a second arm: `\( -path "*/logic/*" -o -path "*/viz/engine/*" \)`. Proven to bite: a
probe `use leptos;` in `engine/shapes.rs` failed the gate at the new path, and the clean
tree passes.

## b · `Limits` + `GO_JUDGE_LIMITS` leave the shared crate

The step-45 test, reapplied: they carry no serde, never cross the wire, and the client
references them zero times — "shared" described the folder rather than the fact. Both now
live in `server/src/execution/domain/` (a pure value type beside `Language`); four import
sites re-pathed, one of them the cross-context `platform/limits_tests.rs` (test-only,
acceptable — it pins the edge-timeout-outlasts-the-sandbox invariant). The shared crate is
now wire DTOs and the shared judge, nothing else.

## c · `pub` → `pub(crate)` where no outsider exists

The extraction confirmed none of the execution-infrastructure helpers have a `server/tests/`
consumer — every user is a co-located `#[path]` unit test or a sibling production module. So:
`RUN_PATH`, `build_request_body`, `parse_run_result`, `Recipe` (struct + fields +
`for_language`), `effective_source`, `JAVA_TRACER_SENTINEL`, `REQUEST_TIMEOUT`, and the
`java_rewriter`/`recipe`/`wire` modules themselves are `pub(crate)` now; tutoring's
`build_request_body`/`parse_reply` too. One nuance clippy caught: the
`GO_JUDGE_REQUEST_TIMEOUT` re-export's ONLY consumer is a `cfg(test)` module, so as
`pub(crate)` it was dead in production builds — it is `#[cfg(test)]` now, which states
exactly what it always was. `GoJudgeRunner` and `OllamaTutorClient` stay `pub` (main
constructs them).

## d · ChromeState picks one delivery path

The reader `provide_context`ed ChromeState AND passed it as a prop to six components, while
four other sites pulled it back with `use_context` — two paths to the same store, and a
consumer had to know which one it was on. All six prop-takers (`ReadingProgress`,
`StickyBar`, `MiniMap`, `TocFab`, `ScrollTop`, `ReaderNavDrawer`) now `expect_context` it —
they render strictly inside `LessonPage`'s tree, so context always reaches them. Context is
the single path; the prop plumbing is gone.

## e · The comment that said OUTERMOST and wasn't

`lib.rs`'s security-headers comment claimed the stamp was outermost; in fact compression,
limits, and telemetry wrap further out (telemetry is the true outermost, per its own step-45
comment). Reworded: outermost *of the application sub-trees*, transport layers outside. No
code change — but a comment that misstates layer order is exactly the kind that costs an
afternoon later.

## Verified

Conventions (incl. the new gate arm + bite test) · fmt · clippy pedantic `-D warnings`
(clean — the `cfg(test)` fix was its catch) · `cargo test --workspace` 458 · vitest 83.
Live at :5373: all six chrome components mounted through context on a prose lesson (progress
bar, sticky bar, 10 minimap ticks, TOC FAB, scroll-top, drawer FAB); the Visualise modal
traced 15 steps through `decoder`/`shapes` from their new engine home; zero console errors.
