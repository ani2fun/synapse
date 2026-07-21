# A10 — The viz wasm island (risk 2)

The one step where two build systems consume one artifact. The viz slice — the pure engine, the
SVG render families, the trace session, the Visualise modal — left the client crate and became a
workspace crate of its own, **`viz-wasm/`**: an rlib to the old Leptos client (its `viz` module,
repointed, nothing inside changed) and a cdylib to the Astro app (a standalone lazy wasm bundle
behind a three-verb wasm-bindgen surface).

## What moved

`git mv client/src/viz → viz-wasm/src` (engine, render, host, modal, session, transport,
registry, blocks), `client/tests/{adapt_stages,cortex_goldens,fixtures} → viz-wasm/tests`, and
the tracer harnesses `client/islands/tracer → web/src/lib/islands/tracer` (the @editor/@markdown/
@diagram single-sourcing pattern — the old client's `@tracer` alias repoints there). The 93
engine tests + 16 goldens + the golden gate now run as `cargo test -p viz-wasm`; the conventions
purity arm followed the engine to `viz-wasm/src/engine/` (bite-tested — a planted `use leptos`
fails the gate).

## The six couplings, replaced in-crate

The step-58 survey found exactly six outward references, and each got the smallest honest
in-crate replacement: the **mount-kit slice** (`mount.rs` — `elements` + `mount`, deliberately
WITHOUT the store caravan: viz owns no app context), the **editor and tracer externs**
(`ffi/editor.rs`, `ffi/tracer.rs` — the extern declarations travel with the crate so
`@editor/loader`/`@tracer/loader` resolve through WHICHEVER Vite bundles the glue), a **minimal
`/api/run`** (`api.rs`, with its own bearer seam — `Box<dyn Fn>` rather than the client's plain
`fn` pointer, because the Astro host's provider closes over a `js_sys::Function`), the **theme
probe** (`theme.rs` reads the `dark` class both hosts stamp), and the **logger** (`log.rs`,
verbatim). The old client's `AuthStore` installs its token into BOTH seams now — one line.

## The entry surface

`entry.rs` exports three verbs: `viz_mount_widgets` (body-wide discovery), `viz_open_modal`
(takes the RAW `viz=` hint; `VizStructure::parse` splits structure + root exactly as the old
Visualise button did; unknown hint → logged refusal, not a doomed modal), and
`viz_install_token`. Self-hosting is the step-39 lesson applied forward: the modal store is
minted under a DETACHED root owner and the modal mounts into its own document-level host with
the store provided as context — `modal.rs` runs unchanged under both hosts. Widget handles leak
into a thread-local by design: the Astro app is an MPA, page lifetime IS wasm-instance lifetime.

`blocks::mount_widgets` became **marker-idempotent** (`data-viz-mounted`): the Astro host sweeps
at load AND whenever late-rendered markdown (the editorial pane) plants fresh widgets — the
second sweep must skip live widgets, not stack mounts inside them.

## The lazy loader

`web/src/islands/viz.ts` is the only code that touches the bundle, and it decides when a page
pays the 281 KiB gz: planted `.viz-widget`s → load when the FIRST one nears the viewport (600px
margin, same as lazy Monaco — and `display:none` never intersects, which the tour exploits);
no widgets but a workbench variant with a `viz=` hint → load on idle so the Visualise button
appears unprompted; neither → not a byte. Two new window contracts: `VIZ_READY` (the button is a
render-time `__synapseViz` check — the event re-renders mounted workbenches) and `VIZ_RESCAN`
(MarkdownPane dispatches after every late render; the loader re-sweeps, or loads first if the
rescan is the page's first reason). The bearer is indirected through `window.__synapseVizToken`
read per-request, so identity (A11) and the wasm can arrive in either order.

## The tour, re-homed

`components/Tour.astro` + `islands/tour.ts` replace A04's static hero with the four-slide
carousel (7s auto-advance, hover-pauses, wraps, dots + arrows + NN/04 label). The Rust
re-rendered one slide per index; here all four SSR (2–4 `display:none`) and the island only
toggles visibility — and the library cards' hrefs resolve at REQUEST time from the index the
page already fetched, better than the client's reactive fallback. Slide 4 is the REAL widget as
a planted `.viz-widget` whose payload is the hand-authored two-pointer reverse serialized to the
tolerant authored wire (bare `{title, steps}`, string annotations) — and its lazy load costs the
landing nothing until the tour actually reaches it.

## CI and the old client

The `web` job builds the wasm before `astro build` (dev profile — the job proves the graph
RESOLVES; size is A12's budget gate) — the pkg is gitignored build output and Vite cannot bundle
a bundle that was never built. `dev-tools/dev` builds it when absent for the same reason.
`server/tests/java_tracer_it.rs` follows the harness to its new path. The old client builds
green at **673/700 KiB gz** — up ~36 from the crate boundary (cross-crate generic sharing lost
to separate instantiation), accepted without a fight: the old client is deleted at A14, and the
Astro path ships the same code lazily instead of always.

## Verified live (both faces)

- **Modal path** (gallery lesson, real go-judge): 18 Visualise buttons appeared via VIZ_READY;
  click → modal → real trace → Ready with 15 timeline steps, SVG canvas, frames panel; ←/→
  stepping moved the active tick; the source-pane Monaco mounted THROUGH THE CRATE'S OWN FFI
  (the load-bearing risk); Esc closed. Zero console errors.
- **Inline path** (the tour): landing loads NO wasm (`performance` entries pinned); slide-4 dot
  → wasm fetches → widget mounts (5 cells, cursors, authored caption, transport bar); arrows
  wrap. A stale-pkg lesson en route: the packaged wasm predates an edit unless rebuilt — the
  "mounted 1" log against a missing marker was the tell.

## Numbers

479 cargo (111 of them now `-p viz-wasm`) · 184 web vitest · 18 client vitest · 7/7 e2e ·
old client 673/700 KiB gz · viz bundle 281 KiB gz wasm + 8 KiB glue, lazy on every page.

## The lesson

**A seam is only proven when both sides of it are strangers.** The six couplings looked like a
clean boundary for four steps; the crate move is what made them load-bearing — the externs had
to travel, the token seam had to stop being a `fn` pointer, the modal had to learn to host
itself. Nothing inside the engine, the renderers, or the modal changed, and the goldens never
flickered: the work of this step was entirely at the edges, which is exactly what the step-45
extraction of the engine (and step-58's coupling survey) bought in advance.
