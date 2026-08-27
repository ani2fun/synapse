# RS009 — D2 figures are drawn on demand by a sidecar, not committed by CI

**Status:** accepted · 2026-08-27 · **supersedes [RS007](rs007-ahead-of-time-d2.md)**

## Context

RS007 moved d2 rendering out of the request path and into each content repository's CI: a workflow
compiled every ```d2 fence, wrote `_media/d2/<fnv1a(source)>.svg`, and committed it; the reader
hashed the fence and inlined the file. That fixed what it set out to fix — the pod stopped loading
the engine and a diagram-heavy lesson stopped costing the reader 5.9 MB gz.

It also made publishing a diagram a three-step affair. Editing one character of a fence mints a
different hash, so the figure is *not drawn* until CI runs and commits, and until then the page
falls back to the client renderer — silently, by design. The author's own loop was the worst case:
edit, refresh, and get a blank card that is indistinguishable from a broken one, with no CI in
sight to fix it.

The failure mode was structural, not incidental. Every part of RS007 exists to keep two
implementations of one hash in agreement — `dev-tools/render-d2.mjs` carries its own copy of
`fnv1a`, the salt, the layout and the render options because it runs from a plain Node checkout in
another repository, and `renderD2Script.test.ts` exists solely to pin them together. RS007 says it
plainly: *if either half drifts, every lookup misses — silently.*

### What changed

RS007 ruled out server-side rendering on a measured **5252 MB** peak RSS against a 256Mi container.
That number is a property of **the wasm build under Node in-process**, where Go's linear memory
never shrinks — not of d2. Measured on the same 23-diagram lesson, native:

| | peak RSS |
|---|---|
| wasm under Node, in the page tier (RS007) | **5252 MB** |
| native engine, its own container | **160 MB** of a 384Mi limit |

~33× less, and in a container whose limit is its own. The reason RS007's conclusion held was never
"d2 is expensive"; it was "this embedding of d2 is expensive, in a process that cannot afford it."

## Decision

**A `d2-render` sidecar draws fences on demand, caching by content. The page tier asks it while
rendering a lesson and inlines the SVG.**

- `d2-render/` — a Go service on `oss.terrastruct.com/d2`, one long-lived process with the engine
  serialised behind a mutex, and a content-addressed disk cache. Its own image (~33 MB) and its own
  container, so a render spike cannot spend the page tier's 256Mi.
- `SYNAPSE_D2_RENDER_URL` names it. **Unset is the kill switch**, and it restores exactly the
  client-rendered floor — the address *is* the switch, so there is no configuration in which a
  renderer is named and ignored. Deliberately a new name: `SYNAPSE_D2_PRERENDER` has already meant
  two different things and RS007 records that a rollback across `0c50378` must turn it off first.
- A miss stays silent and non-fatal, exactly as before: a sidecar that is down, slow, or handed a
  diagram it cannot parse falls back to the client. `d2-prerender.spec.ts` asserts both modes and
  each asserts the other's markers are absent.

### Why on demand rather than at publish time

Because the cost that justified pre-computing it is gone. A figure is ~70 ms warm and the cache is
immutable by construction — the key *is* the content, so nothing is ever invalidated and a given
diagram is drawn once, ever, for the life of the cache. Pre-computing a value that cheap bought
nothing and cost a CI workflow, a commit, a generated artifact in every content repository, and a
class of silent failure.

### The two engines, and the gate between them

The client keeps `@terrastruct/d2` for `/d2`, `/mermaid` and the authoring preview, which compile
on every keystroke and cannot round-trip. So two engines render the same source, and when they
drift the editor lies about what will ship — quietly.

`dev-tools/d2-engines-agree.mjs` is the gate: it renders every fence in a checkout through both and
compares. **It caught a real drift immediately.** Pinning the module to the d2 **v0.7.0 tag** — the
version the wasm build reports — emitted three extra `<mask>` rects on any diagram with a
multi-line label, because the tag carries a bug fixed just after it. On **v0.7.2** the two agree
across all 101 fences of `synapse-content` and `system-design-guide`, to the byte, apart from
`data-d2-version` and d2's own content-hash id (which differs on 9 of them while the element set
stays identical, and is internally consistent in each document).

## Consequences

- **Publishing a diagram is editing a fence.** No CI run, no commit, no artifact. The dev loop and
  production behave identically, which is what makes the loop trustworthy.
- **`_media/d2/` and the `render-d2.yml` workflows are retired**, and with them the *pipeline*
  RS007 built: nothing is generated at publish time and nothing is committed.
  `dev-tools/render-d2.mjs` itself SURVIVES, as the fence lexer and the shared constants — the
  engine-agreement harness and `d2-interactive.mjs` both read fences out of a checkout, and doing
  it a second way is exactly the drift this ADR is trying to avoid. `renderD2Script.test.ts`
  survives with it and still earns its place: it pins that lexer against remark, which is the half
  of RS007's contract that a trailing newline once broke.
- **A new failure mode replaces the old one**: the sidecar is now on the page's critical path for
  first paint of a figure. It is bounded — a 5 s budget per document, then the client floor — and
  it fails toward the behaviour the site had before RS007 rather than toward a broken page.
- **Cold start costs a little.** 23 diagrams took 6.9 s to draw and 0.3 s to serve from cache
  afterwards. A pod restart re-pays that once, spread across whoever reads first, and only because
  an `emptyDir` is the cheapest volume that is correct.
- **Walkthroughs (```d2 boards) moved too, and got simpler for it.** `POST /boards` compiles once
  and draws EVERY board into the cache, returning the graph and the root; the reader fetches the
  rest through `GET /api/synapse/d2/{hash}/{slug}`, a small reverse proxy in the page-serving tier
  built on the same pattern as `/c4`. Because a walkthrough is now addressed by what it IS, the
  lesson dropped out of the addressing entirely: `RenderContext`, the `?lesson=` query, the
  `_d2/<fence>/` sidecars, `boards.json` and the `BoardFile` allowlist in the catalog domain are
  all retired. The same walkthrough in two lessons is one set of boards.
- **The board model is now implemented twice**, in TypeScript for the viewer and in Go for the
  renderer — `boardSlug`, `fnv1a` and the walk. That is the shape RS007 warned about, so it is
  pinned the same way: the Go tests assert against vectors taken from the TypeScript, and `fnv1a`
  hashes UTF-16 code units because ranging over a Go string yields runes and this catalog's labels
  are full of `·` and `→`.
