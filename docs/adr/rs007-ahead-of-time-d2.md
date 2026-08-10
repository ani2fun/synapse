# RS007 — D2 figures are drawn in the content repository's CI, not at read time

**Status:** accepted · 2026-08-10

## Context

A ```d2 fence rendered in the reader's browser. `renderLesson` emitted a placeholder carrying the
URI-encoded source and `Diagrams.tsx` compiled it at mount through `@terrastruct/d2` — a Go
diagram engine compiled to WebAssembly. The design was deliberate and documented as "prose-first":
the lesson's text painted immediately and the diagrams caught up.

They did not catch up. Measured against production on a **warm** HTTP cache, on the 23-diagram
lesson `/synapse/synapse-features/reading-a-lesson/d2-diagrams-snippets`:

| | |
|---|---|
| `load` event — prose is up | 420 ms |
| diagrams rendered at 12.9 s | **0 / 23** |
| all 23 rendered | **~34.9 s** |

A 150 ms polling loop was starved for a continuous 20 s inside that window, so the main thread was
blocked rather than merely busy. A cold reader additionally downloaded **5.9 MB gz / 7.8 MB
decoded** before any of it began.

The cause was not the engine's speed. `renderD2Source` constructed `new D2()` **per diagram**, and
that constructor is not a cheap handle: it base64-decodes a ~5.5 MB string, brotli-decompresses it
to a **21 MB** wasm binary **on the main thread**, spawns a worker, structured-clones the binary
into it, and instantiates it alongside a Go runtime. Twenty-three fences did that twenty-three
times and left twenty-three workers alive, because the package exposes no teardown.

The per-instance shape existed for a real reason — a shared instance deadlocks at three concurrent
compiles — but the diagnosis was incomplete. The worker binding tracks a **single in-flight
request**, one resolve/reject pair overwritten on every send, so overlapping calls clobber each
other's continuation. That is a missing request queue, not a need for N engines.

### The obvious fix, and why it failed

With one pooled instance behind a serialising queue, compiling during SSR became affordable —
apparently. It shipped, and it took the site down. The engine's cost is not the module, it is the
run:

| sidecar RSS | |
|---|---|
| idle | 83 MB |
| **peak, rendering the 23-diagram lesson** | **5252 MB** |
| settled afterwards | 880 MB |

against a **256Mi** container. The pod OOM-killed mid-response, the connection dropped, and the
edge turned that into a 502 — on exactly the diagram-heavy lessons the work was for, while every
other page stayed green, because nothing else loads the engine. That asymmetry is what let it
reach production at all, and the e2e cannot catch it: the suite runs against a dev-machine sidecar
with no cgroup limit.

Raising the limit was never the answer. The peak is **20×** the whole container and the settled
figure is still 3.4×.

## Decision

**A content repository compiles its own ```d2 fences in CI and commits the SVG. The reader's page
looks that file up and inlines it.**

The expensive half runs where memory is free and only when the prose changes; the serving half
does a file read.

### The convention

- `dev-tools/render-d2.mjs` walks a checkout, compiles every d2 fence, and writes
  **`_media/d2/<fnv1a(source)>.svg`**. A repo's CI runs it on any `.md` push and commits the
  result.
- **Content-addressed, not namespaced per book** — unlike the rest of `_media`, which follows
  `_media/<book-slug>/…`. Two repositories that author the same diagram produce the same filename
  *and the same bytes*, so `/media`'s first-wins probe over every mounted checkout cannot pick a
  wrong one. Naming by content is also what makes the run idempotent: editing one lesson redraws
  one file, and an unchanged diagram is never touched.
- `markdown/d2Prerender.ts` hashes the fence, fetches `{api}/media/d2/<hash>.svg`, and inlines it,
  behind a bounded LRU. A response that is not an SVG is treated as a miss, never as a figure.
- **Inline, not `<img src>`.** An SVG loaded as an image is inert, and these carry `<a href>`
  links and `<title>` tooltips authors rely on. Inlining also makes the links work before any JS
  loads — better than the behaviour it replaces.
- A **lone** fence is inlined. A run of adjacent fences is a slideshow and gets **slide 0 only**:
  that is the one its transport paints at mount, so inlining it removes the eager engine load,
  while inlining all N would ship figures nobody has stepped to.
- The file is drawn with the salt of a diagram's *first* occurrence. A document that repeats one
  verbatim gets the second copy re-salted at inline time, so element ids stay unique in a page.

### Why the renderer lives in this repository

The script names a file after `fnv1a(source)` and draws it with a specific layout engine, theme,
padding and salt. The reader looks up the same hash and expects the same options. **If either half
drifts, every lookup misses — silently**, because a miss falls back to the browser and merely gets
slow. There is no error to see.

The script cannot import the app's copies: it runs from a plain Node checkout inside a content
repository, with no TypeScript. So it carries its own, and `renderD2Script.test.ts` pins the two
together — hash, salt, layout, render options, and the fence lexer.

That test earned itself immediately. The script lexes fences with a regex; remark's `code` node
carries **no trailing newline** and a regex over the fence keeps one. Every hash differed and 0/23
figures resolved. The first draft of the test asserted the *script's* output was correct, which
merely encoded the bug — it now parses the same markdown with remark and compares against that.

Content repositories borrow the script by checking out `ani2fun/synapse`, the same way their
`validate.yml` already borrows the server's own walker. This repository deliberately dropped a
cross-repo `uses:` reference once, after renames made it resolve somewhere unexpected; a checkout
of a named path fails loudly instead of quietly running a lookalike.

### The client renderer stays

It is the fallback, and it is reached often enough to matter: a fence newer than its repository's
last CI run, a repository with no workflow yet, a slideshow's later slides, and the authoring
preview, which runs `renderLesson` in the browser where no lookup is possible. It keeps the shared
instance and the queue from the failed SSR attempt, plus near-viewport gating and an in-memory
cache — which is why even the un-drawn path is now **2.96 s** to first diagram instead of tens of
seconds, with only the near-viewport diagrams compiling at all.

### The kill switch

`SYNAPSE_D2_PRERENDER=off` restores the fully client-rendered behaviour without a deploy. Its
**meaning changed with the image**: from `0c50378` onward `on` means "read the drawn file", and in
any earlier image the same value means "compile here" — the 5.2 GB path. A rollback past that
commit must turn it off first. This is recorded beside the variable in `ani2fun/infra`.

## Consequences

- **The pod never loads the d2 engine.** The prod image drops `@terrastruct/d2` entirely again
  (~950 MB), and serving a fully-drawn 23-diagram lesson measured **147 MB** RSS against 5252 MB.
- **Publishing stays a `git push`.** Drawing is part of the content repository's own CI; the app
  deploys nothing to gain a diagram.
- **Lesson HTML grows.** An inlined figure costs roughly **4–5 KiB gz**; the 23-diagram lesson is
  ~130 KiB gz, against ~5.9 MB gz of engine it no longer sends. Note the trade is not free bytes:
  the engine chunk was `immutable`, so a returning reader used to download nothing and pay 35 s of
  blocked main thread instead. This spends ~130 KiB per navigation to buy that back.
- **Every content repository needs the workflow.** Six satellites plus the spine carry it; a
  repository without it degrades to client rendering, correctly and silently.
- **Failure is invisible by construction**, so it is gated rather than watched:
  `e2e/tests/d2-prerender.spec.ts` asserts the SVG is in the response *body* and that no asset
  over 2 MB is fetched, and it asserts **both** modes — the suite's main pass runs the prod shape
  and re-runs that file with the lookup on, each checking the other's markers are absent. The
  sidecar logs `d2: N/M figure(s) inlined from _media/d2` and warns on every fallback.
- **Rendering is deterministic across machines** — verified by deleting a CI-drawn figure,
  redrawing it locally, and comparing bytes. It has to be: the hash keys the *source*, so a
  divergence in *output* would be invisible.

## Alternatives rejected

- **Compile during SSR.** Tried, shipped, caused an outage. 5.2 GB peak against 256Mi.
- **Raise the memory limit.** 20× is not a nudge, and it would buy a pod sized for its worst
  lesson forever.
- **Draw at image build time.** Content is git-synced independently of the image; a figure would
  be stale the moment a lesson changed, which is the property RS005 exists to protect.
- **A cluster job writing into the content volume.** Works, but adds a scheduled component and a
  writable shared volume to own — the content repository's CI already runs on every change and
  already has the memory.
- **Serve the SVG as `<img src>`.** Loses `<a href>` links and `<title>` tooltips, which this
  catalog's diagrams use.
- **Vendor the render script into each content repository.** Removes the cross-repo checkout, but
  the copies drift and the drift is silent. The single source with a pinning test is the safer
  coupling.
