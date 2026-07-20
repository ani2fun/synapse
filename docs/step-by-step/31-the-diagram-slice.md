# Step 31 — The diagram slice: mermaid, d2, LikeC4 — all rendering, one zoom

*(oracle: the step 24–25 diagram arc — `mermaid.ts`, `DiagramBlocks`, `MermaidView`/`D2View`,
`DiagramZoom`, diagrams.css.)*

## What was broken

The markdown pipeline (step 08, oracle-verbatim) has been emitting diagram placeholders all
along — `.mermaid-block[data-source]`, `.d2-block[data-svg]`, `.d2-slideshow[data-slides]` —
but nothing hydrated them: authored diagrams rendered as empty divs (two visible in the
`low-level-design` book alone), and the LikeC4 lesson iframe had no `/c4` route in dev.

## The mermaid island

`islands/diagram/mermaid.ts` is the oracle's, verbatim: mermaid@11, `securityLevel:
"strict"`, always the LIGHT `"default"` theme (authored diagrams colour nodes with light
pastel fills and never set text colour — the dark theme would paint light-on-light),
`fontFamily: "inherit"`, a monotonic render id. The loader is the RS flat-FFI shape
(`renderMermaid(target, src)` folds the lazy import in); the multi-hundred-KB chunk lands
only on lessons that actually carry a diagram.

## Cards + hydration

`catalog::view::diagrams::hydrate_diagrams` mounts per placeholder: `MermaidCard` (island
render; a malformed diagram becomes the loud `.diagram-error` card with the raw source —
never a blank figure), `SvgCard` (d2's parse-time SVG injected), and `D2Slideshow` (a run of
adjacent d2 fences steps through one figure with the ‹ i / n › transport). Every figure sits
on a FIXED-LIGHT card — the authored palettes assume light — capped at `min(70vh, 32rem)` so
inline diagrams stay glanceable.

**One surface escapes that card, and it is not SVG.** Both engines emit their title into a
`<foreignObject>`, which contains real HTML — so `.synapse-prose h1 { color: hsl(var(--foreground)) }`
reaches it and the title alone follows the *page* theme. In dark mode it computed to
`rgb(231,231,228)`: near-white ink on a permanently white card, invisible. The fix pins
`.diagram__figure foreignObject :is(h1…h6, p, span, div, li, td, th, code)` to the light
foreground as a literal, not a token — a token would track the theme again, which is the bug.
It cannot touch the diagram's own palette: shape labels are SVG `<text>` carrying their own
`fill`, and `fill` does not inherit from `color` (verified unchanged before and after).

This is the same light-on-light hazard the mermaid island's LIGHT-theme pin already guards
against, one level up. The guard was written for *fills* and the title is *text* — so a fixed
card plus a themed page means every text surface must be audited for which of the two it
inherits from. There is one known remainder: d2 sizes the `foreignObject` for a single line
(`height="51"`) while the browser wraps a long title to two (needs 90px), so an over-long
title clips. `overflow: visible` is not the fix — it needs 39px against 20px of headroom and
would collide with the first shape; growing the `foreignObject` during hydration is.

## The error path, and the orphan it used to leave

The error card is only half the story, because mermaid renders an error of its own. `render()` is
called without a container, so mermaid appends its working node — `#d<id>` — to `document.body`
to measure in. On failure it draws its "Syntax error in text" bomb graphic into that node and
then throws **before** reaching its own `removeTempElements()`. The node stays in `<body>`.

Nothing full-page-reloads in a CSR app, so that orphan outlives every client-side navigation: one
failed diagram, anywhere, pins a bomb to the bottom of every page for the rest of the session —
including the landing page, which has no diagrams at all. It also *accumulates*, because our ids
are monotonic (`synapse-mermaid-N`) and mermaid's own `removeExistingElements` only clears the
id it is about to reuse. Measured in the real renderer: three failures → three bombs (`1, 2, 3`),
each one the last child of `<body>`.

`suppressErrorRendering: true` fixes it, and it is the right flag rather than a workaround: it
routes mermaid's error branch through cleanup-then-throw instead of draw-then-throw. We still get
the rejection that drives `MermaidCard`'s error card, mermaid stops drawing a second, worse error
we never asked for, and `<body>` is left as it was found. A defensive `#d<id>` removal in the
island's `catch` covers any future internal path that throws before cleanup.

Worth knowing when reading a bug report: **that bomb does not mean the diagram has a syntax
error.** `errorRenderer` is what mermaid draws for *any* failure inside `renderer.draw`, so a
lazily-loaded diagram-type chunk that 404s — which is exactly what a deploy does to a tab holding
the previous build — produces the identical "Syntax error in text" graphic over perfectly valid
source. All 196 authored diagrams parse *and* fully render clean, so a bomb in the wild is a
runtime failure, not an authoring one.

## The zoom overlay — chrome on the LEFT

A rendered figure grows the ⤢ Enlarge pill (top-LEFT, hover-revealed); it opens the
near-fullscreen `.diagram-zoom` overlay: light paper card over a blurred scrim, wheel zoom +
drag pan on the viewport, the − % + ⟲ pill bottom-centre, ✕ Close **top-LEFT**, Esc/scrim
close. The house rule is deliberate and now uniform: OUR chrome — the card Enlarge, the
overlay Close, the practice widget's Enlarge — sits top-left, because LikeC4's own chrome
(✕ · Share · Export, per-node tools) owns the top-right corner.

## LikeC4

The lesson embed is an authored `<iframe src="/c4/view/…">`; the dev seam was missing on
both ends: vite now proxies `/c4` to the server (which fronts the compose `likec4` service —
the oracle's opt-in `--profile c4` builder over the merged synapse-content workspace), and
the iframe gets a rounded dark frame. Verified: the viewer SPA boots inside the iframe
(react-flow mounted) on the Architecture Docs lesson.

## Verified live

`java-basics`: both mermaid diagrams render (0 errors), Enlarge → overlay with the SVG,
+ → 125%, Close at (14, 12) top-left, Esc closes. `storage-engines`: the 3-slide d2
slideshow steps 1/3 → 2/3 plus a single diagram card. `architecture-docs`: the LikeC4
viewer live through `/c4`. Suite: 347 Rust + 44 vitest; bundle 557/700 KiB gz (mermaid is
a lazy chunk, off the critical path).

Next: RS-P8 continues — the landing tour + hero, then the mobile drawer + LikeC4 fullscreen
chrome, then architecture docs + capstone.
