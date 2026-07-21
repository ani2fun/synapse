# A05 — The palette and the reader chrome, live

*(the seven specs were written in step 52, against the OLD Leptos client, and have sat in the
repo ever since as the acceptance contract for whatever front end finally passes them. This is
the step where they turn green against the new one — the milestone the whole migration has been
building toward since A01's off switch.)*

> Branch chapter: the Astro migration runs on branch `astro`, numbered A01–A14, folded into the
> main ledger at merge. Main is at step-65 and keeps moving underneath.

## What this step is for

Global chrome: the ⌘K search palette, the interactive reader sidebar (done-ticks, the mobile
drawer), and reading-progress WRITES. A01–A04 built SSR pages that read like the old client;
this step is the first one that makes the Astro front end *behave* like it too — a reader can
search, finish a lesson, and see that they finished it, all without a wasm compiler on the
critical path.

**The milestone: all 7 existing e2e specs (`reader.spec.ts` × 5, `mobile.spec.ts` × 2) pass
locally against the Astro serve, unmodified.** They are the acceptance contract, not a checklist
to satisfy by editing — nothing in this step touched either spec file.

## The two ports

`client/src/search/logic/mod.rs` → `web/src/lib/search.ts` (+ `search.test.ts`, all 5 cases from
`logic_tests.rs`, case names turned to camelCase): flatten the catalog + blog list into
`SearchEntry`s, rank prefix (100) > word-start (80) > substring (60) > subsequence (30), +10 for
matching the label over the breadcrumb, kind as the tiebreak (lesson > book > blog), shorter
labels first. Reuses A02's `Page`/`pageUrl` from `routes.ts` rather than re-deriving URL
formatting a second time.

`client/src/catalog/logic/prefs.rs` → `web/src/lib/catalog/prefs.ts` (+ `prefs.test.ts`, both
cases): the four-field `size|leading|family|width` pack, exact-arity parse with per-field
canonical fallback, `DEFAULT_PREFS`. The Rust oracle splits this pure module (`logic/prefs.rs`)
from its state-layer twin (`state/mod.rs`'s `apply_to_html`); TS folds the reflect-onto-`<html>`
step in as `applyToHtml` rather than inventing a second file for one four-line function — there
is no separate state-layer file on this side, and the FAB *editing* UI it would belong to is
deferred anyway (see below).

Both ports found zero divergence from their oracle: same fixtures, same assertions, first run
green.

## The palette island

`web/src/islands/palette.ts` — vanilla TS, no framework, built directly off
`client/src/search/{state,view}/mod.rs` as the spec rather than the Rust JSX-shaped view tree.
A `Palette` class owns `isOpen`/`query`/`selected`, builds the `.cmdk-scrim` > `.cmdk` >
`.cmdk__input` + `.cmdk__results` DOM tree ONLY while open (mirrors the Leptos
`store.is_open.get().then(...)`, not a CSS-hidden node sitting in the document from load), and
tears it down on close — so Playwright's `toBeHidden()` after Escape is genuinely "not in the
DOM," not "present but invisible."

Data loads lazily: `fetchIndex()` + `blogList()` fire on first `open()`, cached in a module-level
variable for the rest of the page's life, and a blog failure degrades to an empty blog slice
rather than losing the lessons/books (mirrors the Rust memo's `match blog.list().get() { Loaded
=> …, _ => logic::entries(&index, &[]) }`). Typing before the fetch resolves searches against
whatever's cached so far (usually nothing yet); the in-flight `refresh()` re-renders once the
fetch lands, reading `this.query` at THAT later moment — so a query typed while loading is not
lost, it just resolves once the data does.

Wiring: the header's `.header__search` button (shipped inert since A03) now opens it; a global
`window` keydown listens for `Ctrl+K`/`Cmd+K` from any page; arrows + Enter navigate the result
list; a real `<a href>` on every row means clicking a result is an ordinary page load — there is
no client-side router to hand off to. Mounted from `Base.astro` (`<script>import
"../islands/palette"</script>`, right beside `Header`/`Footer`), so it exists on **every** page —
the palette e2e spec opens it from `/`.

### The bug this step found: a stylesheet nobody had ever imported

The palette's DOM built correctly and `.cmdk__input` was visible — but at the wrong position.
`box.y` came back `810` against a 720px-tall viewport, i.e. rendered in normal document flow at
the page's bottom, not centered as a fixed overlay. This is **exactly** the step-40 scar the
spec's own comment warns about ("the palette rendered at the page's bottom-left … assert it is
actually placed, not merely present") — but a different root cause with the identical symptom.

The actual cause: `client/styles/search.css` — the file holding every `.cmdk*` rule, including
the load-bearing `.cmdk-scrim { position: fixed; … display: flex; justify-content: center; }` —
was never imported anywhere under `web/`. `Base.astro` imports `tokens.css` + `shell.css`;
`index.astro` imports `library.css` + `tour.css`; `[...path].astro` imports `reader.css` +
`markdown.css`. Nobody had reason to reach for `search.css` before this step, because nothing
under `web/` rendered anything with those classes until now. Fixed with one import in
`Base.astro`, alongside a comment naming the step-40 precedent so the next person who sees a
misplaced fixed-position overlay checks "is the stylesheet even loaded" before "is the CSS
wrong."

## The reader island

`web/src/islands/reader.ts` — also vanilla TS, mounted only on lesson/problem pages
(`[...path].astro`'s `<script>import "../../islands/reader"</script>`, inside the `isLesson`
branch). Four jobs, each a straight port through A04's pure `progress.ts`/`prefs.ts` rather than
re-derived:

- **Done-ticks** (oracle: `sidebar.rs`'s `lesson_link`): on load, read the finished set
  (`progress.parse` over `storage.get(READER_PROGRESS_KEY)`), walk every `.reader-sidebar__link`
  in the document, and for the ones whose `href` resolves to a done path, add
  `.reader-sidebar__link--done` + a `<span class="reader-sidebar__tick" aria-label="Finished">✓</span>`
  — the exact class list and markup `sidebar.rs` renders reactively, applied once, imperatively,
  against whatever Astro already put in the DOM.
- **Progress writes**: `visit(path)` records `reader-last` with skip-if-unchanged semantics
  (`ProgressStore.visit`'s `if last == path { return }`) on every lesson-page load. A scroll
  listener recomputes `track = scrollHeight - innerHeight` / `scroll = scrollY` and calls A04's
  `isAtEnd` (ported from `progress::is_at_end` — the `track <= 0` short-lesson trap and the
  non-finite-ratio trap both live there already); the first match marks the lesson done, once,
  through the same idempotent-set logic `ProgressStore.set_done` uses (a re-mark of an
  already-finished lesson writes nothing). Unlike the Leptos SPA, there is no "reset per lesson"
  concern here — Astro has no client-side router, so every lesson is a fresh full-page load and
  the script starts clean every time.
- **The mobile drawer** (oracle: `reader.rs`'s `ReaderNavDrawer`): `[...path].astro` now renders
  a `.reader-nav` container (a sibling of `.reader-layout`, OUTSIDE the grid — the same step-38
  prod-bug rule the oracle chapter names, re-stated in a comment at the call site so it isn't
  rediscovered) holding the `.reader-nav-fab` button, CSS-hidden at ≥1024px exactly like the
  desktop sidebar it replaces. On click, `reader.ts` builds `.reader-nav-scrim` +
  `.reader-nav-drawer` (head: `.reader-nav-drawer__title` "Contents" + `.reader-nav-drawer__close`
  "✕"), fills the drawer body with a **clone** of the already-rendered
  `.reader-sidebar .reader-sidebar__inner` (done-ticks re-applied to the clone, though cloning
  after the original was ticked already carries them over — belt and suspenders), and closes on
  scrim click, Escape, or any click landing on/inside an `<a>` (`target.closest("a")`, letting the
  link itself still navigate). `z-index` ordering is untouched CSS (`reader.css`'s existing 55/56
  above the header's 50) — the mobile spec's `elementFromPoint` assertion on the close button
  passes because nothing here fights that stacking order.
- **Prefs**: `applyStoredPrefs()` parses `reader-prefs` and reflects the four `data-reader-*`
  attributes onto `<html>` on load, so a saved size/leading/family/width choice still applies to
  the prose. The FAB's *editing* popover is NOT built this step (see deferrals) — this is only
  the "apply what's already saved" half, which the chapter brief called out as cheap where the
  editing UI is not.

## What deliberately does not work yet

None of these are exercised by the seven e2e specs, and the SSR sidebar/reader chrome never
rendered their markup in the first place — there is nothing half-wired to leave behind:

- **The Compact rail and the sidebar's Hidden face.** `SidebarMode` persistence
  (`reader-sidebar` key), the collapse-to-rail / hide-entirely controls, and the rail's
  conic-progress ring. `Sidebar.astro` renders the Expanded face only, unconditionally.
- **The sidebar filter box and the Learn-browse toggle.** Neither exists in the SSR markup to
  wire up.
- **The sticky title/section bar, the right-edge minimap, the page-TOC FAB + popover, focus
  mode, and the scroll-to-top FAB.** All chrome that floats over the prose; none is in the e2e
  contract, and building them without the one shared `ChromeState`-equivalent scroll recompute
  they all lean on in the oracle would mean either re-deriving that plumbing now or building four
  one-off scroll listeners. Left for a dedicated polish step.
- **The reading-preferences FAB itself** (the `Aa` button + segmented-control popover). Only the
  "apply a saved choice" half of `PrefsStore` ported — there is no UI yet to make a new choice.
- **`SidebarMode`/sidebar-face persistence.** Not needed: nothing renders a second face to
  persist toward.

## Gates

- `dev-tools/check-conventions.sh` — clean; every new file well under the 800-line cap
  (`palette.ts` 244, the largest of the six).
- `cargo fmt --all --check` · `cargo clippy --workspace --all-targets --all-features -- -D
  warnings` · `cargo test --workspace` — clean; this step touched no Rust (477 tests, unchanged).
- `(cd web && npm test && npm run build)` — vitest **88 → 95** (+5 `search.test.ts` + 2
  `prefs.test.ts`), `astro build` green.
- `(cd client && npm test && npm run build)` — vitest **27** unchanged; the release wasm + vite
  build succeeds, unchanged in shape from A04.

## Verified

```
cargo:  477 tests green (unchanged — no Rust surface this step)
web vitest: 95 tests, 9 files (routes 2, seo 2, render 56, tree 18, progress 10, search 5,
            prefs 2) — 88 → 95
client vitest: 27 tests, 2 files, unchanged
build: web astro build ~800-900ms · client wasm:release + vite build green

e2e (astro front end: playwright → axum :8280 → astro_proxy → node sidecar :4321 → SSR → /api
     back to axum; e2e/fixture-content, real Postgres synapse_rs)

  SYNAPSE_ASTRO_URL=http://127.0.0.1:4321 SYNAPSE_ROOT=e2e/fixture-content \
  SYNAPSE_DATABASE_URL=postgres://synapse:synapse@localhost:5532/synapse_rs SYNAPSE_PORT=8280 \
  ./target/debug/synapse-server   (background)
  PORT=4321 HOST=127.0.0.1 node web/dist/server/entry.mjs   (background)
  cd e2e && E2E_BASE_URL=http://localhost:8280 npx playwright test

  Running 7 tests using 2 workers

    ✓ [chromium] the server renders a per-page head, not the placeholder
    ✓ [mobile]   the reader fits the screen
    ✓ [chromium] the lesson body renders and its prose hydrates
    ✓ [chromium] the page does not scroll sideways
    ✓ [mobile]   the nav drawer opens and its close button is actually clickable
    ✓ [chromium] the command palette opens and navigates
    ✓ [chromium] finishing a lesson is remembered across a reload

    7 passed (2.8s)

  Re-run once more for stability (no retries configured locally): 7 passed (3.0s) again, no
  page errors attached (the `fixtures.ts` harness fails a spec on any uncaught console error or
  pageerror — a clean run here means the palette/reader islands threw nothing across both
  passes).

  First run of "the command palette opens and navigates" FAILED before the `search.css` fix
  above (`box.y` 810 vs the required < 360) — left in this chapter as the actual finding, not
  smoothed over: the palette's DOM and logic were correct from the first write, and the failure
  was a missing stylesheet import, not a bug in `palette.ts`.
```

No spec required editing to pass, and none was.

## The lesson

**A spec that asserts *position*, not just presence, earns its keep exactly once — and this was
that once.** `toBeVisible()` on `.cmdk__input` would have passed the whole time; the palette
class and its DOM were correct from the first draft. It was the spec's second assertion —
`box.y` actually being in the top half of the viewport — that caught a stylesheet nobody had
ever imported, because nothing before this step needed `.cmdk*` rules to exist. The step-40
chapter left a comment in `search.css` warning about exactly this failure mode from the CSS
side (an orphaned rule swallowing the centering declaration); this time the file itself was
never wired to the page at all, on the OTHER side of the same seam — proof that the class of bug
survives a full framework rewrite when the thing that guards against it is a positional
assertion, not the CSS's own good intentions.
