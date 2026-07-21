# A03 — The lesson, server-rendered

*(the migration's whole reason for being is one number: prose that a phone reads in ~1.3 s
instead of 7.2 s. This is the step that moves it — everything before was scaffolding, everything
after is putting the interactivity back.)*

> Branch chapter: the Astro migration runs on branch `astro`, numbered A01–A14, folded into the
> main ledger at merge. Main is at step-65 and keeps moving underneath.

## What this step is for

The Leptos client is 19,722 lines compiled to a **641 KiB gz wasm bundle that must boot before a
reader sees any prose** — measured on production, content readable at **1.25 s on broadband, 7.2 s
on Fast-3G + a mid-range phone**, for lessons whose actual rendered content is ~2 KiB gz. The
reader downloads a compiler to be shown a paragraph.

A03 makes `/synapse/{...path}` server-render the lesson: the prose is HTML **in the response**,
produced by the EXACT markdown pipeline the old client used, with a native per-page head. No wasm
is on the critical path for reading. The interactive placeholders the pipeline emits — workbench,
quiz, diagrams, viz — land inert this step and get hydrated island-by-island later; the reader
sees the words immediately regardless.

## What moved, and what was written

The distinction matters because they carry different risk. **Moving** the pipeline has to preserve
its behaviour byte-for-byte — it is the same code that has rendered every lesson since step 08, and
its 596-line contract test is the proof. **Writing** the page is new surface, so it leans on the
generated types (A02) and the reader's own class contract rather than re-deriving either.

**Moved, with history (`git mv`, all three detected as pure renames — `0 insertions, 0
deletions`):**

- `client/islands/markdown/{render,loader}.ts` → `web/src/lib/markdown/` — the unified · remark ·
  rehype · shiki pipeline (`renderLesson`, 529 lines) and its lazy `loader.ts`. `web/package.json`
  gained the nine npm pins the pipeline imports, at the SAME versions `client/package.json`
  carries (many were already present transitively — Astro ships shiki itself — so `npm install`
  added only 20 packages).
- `render.test.ts` (596 lines, 56 cases) moved with it, **unedited**. It now runs in **web's
  vitest**, not the client's: web's `include` already matched `src/**/*.test.ts`, and the client's
  `islands/**/*.test.ts` no longer reaches a dir that left `client/`. So the 56 cases run in
  exactly one suite — client vitest **83 → 27**, web vitest **4 → 60**, no double-count and none
  lost. Web owns the markdown contract now because the pipeline lives in web now; the test moved
  to sit beside the code it pins.

The one thing the move touched outside the moved files: `client/vite.config.mts`'s `@markdown`
alias, repointed `./islands/markdown` → `../web/src/lib/markdown`. The client's wasm-bindgen glue
still imports `@markdown/loader` (the `islands/markdown.rs` extern), and it still resolves — at
**build** time, through the alias, to the file's new home across the workspace. `cd client && npm
run build` proves it: the release build bundles a `render-*.js` chunk (543 KiB) pulled from `web/`.

**Written:**

- `web/src/pages/synapse/[...path].astro` (163 lines) — the page. Fetches the lesson through A02's
  typed `lesson()`, branches on `frontmatter.kind`, renders the prose column, injects the pipeline
  output with `set:html`, computes the head.
- `web/src/pages/404.astro` — Astro's server-output 404, setting its own status so the axum
  `astro_proxy` copies a real 404 back (the A01 header contract's "sidecar 404 is the site 404").
- `web/src/components/{Header,Footer,Sidebar,SidebarTree,Pager,NotFound}.astro` — site chrome and
  the reader's parts, every class matching `shell.css`/`reader.css`/`library.css` so the
  single-sourced stylesheets (until A14) apply unchanged.
- `web/src/lib/catalog/tree.ts` — `bookPrefix` / `readingOrder` / `bookOf`, ported faithfully from
  `client/src/catalog/logic/mod.rs` because the SSR sidebar needs to find the owning book's tree.
  **A04 owns this module and MUST add the 18 parity tests** `logic_tests.rs` carries — they are
  deliberately not written here; this step needed the walk, not the ledger.

## The head-parity story

The e2e `per-page head` spec asserts on the **raw** response, before any JS — it is what a crawler
sees, and reproducing it is the point of step 50, now on the Astro side. The server computes the
head in `platform/static_routes.rs::render_head`: title `Book · Lesson — Synapse`, description =
the lesson summary (else a fixed default), a canonical `/synapse/{path}`, the Open Graph tags.

The pleasant surprise is that **the lesson payload already carries everything the head needs**.
`LessonPayloadDto.book` is a `BookRefDto` with the book's `title`; `frontmatter` carries the
lesson title and summary. So the Astro page walks nothing — `titleForLesson(payload.book.title,
payload.frontmatter.title)` (the A02 seo port) reproduces `title_for` exactly, and
`payload.frontmatter.summary ?? undefined` lets `Base.astro` fall back to a default description
that is **the same string, character for character, as the server's**. The index is fetched only
for the sidebar's tree, and its failure degrades the sidebar to absent — never the head, never the
page.

The result agrees with the axum side without either side importing the other's code — they agree
because they read the same payload and format it the same way, and the e2e spec is what holds that
true.

## What deliberately does not work yet

- **Problem pages are a prose fallback, not the two-pane workbench.** A `kind: problem` lesson
  renders its markdown (inert placeholders and all) under a clearly-marked notice — *"The
  interactive problem workbench arrives in step A07 of the migration."* The two-pane editor, the
  test runner and the editorial tabs are A07; attempting them here would be building the hardest
  page before its islands exist.
- **The pipeline's placeholders are inert.** `.workbench`, `.quiz-block`, `.mermaid-block`,
  `.d2-block`, `.viz-widget`, `.fence-group` all land in the HTML as empty divs. This is expected
  and fine — the prose around them is readable now, and each family hydrates in its own later step.
- **The header's search and theme toggle do nothing.** The `.header__search` button ships with its
  real markup (it is the e2e selector `reader.spec.ts` clicks) but the ⌘K palette and the theme
  store are A05.
- **The sidebar is structure only** — the reading-order tree with the namesake-collapse rule and
  active marking, but no filter box, no Learn-browse, no compact rail, no done-ticks, and hidden
  below 1024px (the mobile drawer is interactive, so also A05).

## Two things the sanity-check taught (both mine)

Astro stamps a `data-astro-cid-*` attribute on **every** element of a component that has a
`<style>` block, so it can scope its selectors — which is why a naive `grep '<h1
class="reader-prose__title">'` found nothing while the h1 was plainly there as `<h1
class="reader-prose__title" data-astro-cid-…>`. The attribute is harmless; the lesson is that
view-source greps have to allow for it.

And the one that would have been a real bug if I had trusted it: `grep -c 'reader-sidebar__link'`
reported **1** for a four-link sidebar, and for a moment that read exactly like A01's "silently
rendered one book of seven". It was the opposite — `-c` counts *lines*, Astro emits the links on
one line, and `grep -o | wc -l` showed the honest 4 (the 4 chapters, each collapsed to its
namesake lesson, the active one marked). A02's whole argument is that a count is only as good as
what it counts; the sidebar was right, the check was wrong.

## Gates

- `dev-tools/check-conventions.sh` — clean (the new `web/` files are all well under the 800-line
  cap; `[...path].astro` is 163).
- `cargo fmt --all --check` · `cargo clippy --workspace --all-targets --all-features -- -D
  warnings` · `cargo test --workspace` — clean; **477 tests, unchanged from the A02 branch point**
  (this step touched no Rust — only the `@markdown` alias and a comment in `client/vite.config.mts`).
- `(cd web && npm test && npm run build)` — vitest **60** (routes 2 + seo 2 + render 56), build in
  ~790 ms.
- `(cd client && npm test && npm run build)` — vitest **27** (tracer 9 + stylesheets 18); the
  release wasm+vite build succeeds, proving the `@markdown` alias reaches the moved pipeline.

## Verified

```
cargo:  477 tests green (unchanged — no Rust surface this step)
vitest: client 27 + web 60 = 87 total; render.test.ts (56) runs ONCE, in web, unedited
build:  web astro build 790ms · client wasm:release + vite build green (render chunk 543 KiB)

e2e (astro front end: playwright → axum :8280 → astro_proxy → node sidecar :4321 → SSR → /api
     back to axum; real synapse-content, real Postgres):
  cd e2e && npx playwright test reader.spec.ts -g "per-page head"
    ✓ the server renders a per-page head, not the placeholder (888ms) — 1 passed

view-source proof — RAW proxied response for
  /synapse/synapse-features/reading-a-lesson/reading-a-lesson, before any JS:

  <title>Synapse Features · Reading a Synapse Lesson — Synapse</title>
  <meta name="description" content="A quick tour of what a Synapse lesson can render: …"
  <link rel="canonical" href="https://synapse.kakde.eu/synapse/synapse-features/reading-a-lesson/…"
  property="og:title"
  <h1 class="reader-prose__title" data-astro-cid-…>Reading a Synapse Lesson</h1>
  class="lesson-body synapse-prose"        ← the prose column, HTML in the response
  HTTP 200 · a bad path (/synapse/nope/does-not-exist) → 404

  sidebar: 4 links, namesake-collapse applied, active row tracked across two lessons
  problem page (flip-characters, kind: problem):
    <title>Synapse Features · Flip Characters — Synapse</title>   ← per-page head holds
    the A07 notice renders · no two-pane layout · HTTP 200
```

## The lesson

**The migration's payoff is a subtraction, not an addition.** Nothing above makes the reader do
more; it makes the reader *wait for less* — the prose arrives without a 641 KiB compiler in front
of it. The whole rest of the branch is the careful work of adding the interactivity back one island
at a time without ever reintroducing that wait. This step is the pivot: A01 built the off switch,
A02 removed the guesswork from the wire, and A03 is the first one a reader would actually feel — the
one where a lesson stops being something wasm produces and becomes something the server hands over.
```

## Fixed forward (user bug report, 2026-07-21)

The footer rendered on EVERY page — an A03 parity bug. The oracle's rule (`footer.rs`) is that the
footer is landing-page ONLY, precisely because problem pages are a fixed-height two-pane layout
with no page scroll: a footer there stretches the document 177px past the viewport, the window
gains a scrollbar, and every in-pane scroll interaction (the editorial Jump pills, the pane
wheel) gets absorbed by the page instead. `Base.astro` now takes `footer` as an opt-in prop
(default false); `index.astro` is the one opt-in. The symptom surfaced two steps later as
"Jump does nothing" — the cause was here.
