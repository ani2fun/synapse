# A04 — The library and the blog, server-rendered

*(A03 moved the debt into the ledger explicitly — "A04 owns this module and MUST add the 18
parity tests" — rather than letting it evaporate as a TODO nobody re-reads. This is that debt
paid, plus the two pages it was always going to fund.)*

> Branch chapter: the Astro migration runs on branch `astro`, numbered A01–A14, folded into the
> main ledger at merge. Main is at step-65 and keeps moving underneath.

## What this step is for

Three things land together because the first pays for the second and third: `/` (the real
library, not A01's placeholder list) and `/blog` + `/blog/{slug}`, SSR, with the anonymous
progress chrome hydrating on top of the library once the page has painted. Underneath both
pages sits the pure logic `tree.ts` was missing since A03 shipped it walk-only — the debt gets
paid FIRST, in full, before either page leans on it.

## Test-parity debt, paid in full

`logic_tests.rs` carries 18 cases over `client/src/catalog/logic/mod.rs`'s file-scope functions
— not just the three A03 needed (`bookPrefix`/`readingOrder`/`bookOf`), but `findBook`,
`firstLessonPath`, `lessonCount`/`chapterCount`, the C4-click resolver, the sidebar filter, the
minimap spread, the problem-content splitter, and the chapter-scoped problem counter. All 18
port to `tree.ts`, same fixtures, same assertions, case names turned to camelCase
(`reading_order_is_preorder_with_full_paths` → `readingOrderIsPreorderWithFullPaths`) — one
Rust test function, one `it`.

**Divergence found between the existing port and the Rust: none.** A03's three functions were
already faithful; every one of the 18 tests passed against `tree.ts` on the first run, no fixes
needed. The other eleven functions didn't exist yet and were ported fresh, straight off
`mod.rs` — `chapterProblems` needed its private `countingChapter` helper carried over too (the
"a problem chapter of its own" rule: when a lesson's parent directory shares its own slug, the
real counting chapter is one level up, or every `problems/<slug>/<slug>.md` problem reads
"1 / 1").

**Not ported, and left with a note in `tree.ts`'s own doc comment** — `mod.rs`'s three
submodules, none of which `logic_tests.rs` touches and none of which this step's pages need:
`editorial.rs` and `pane.rs` (the problem workbench, A07), `prefs.rs` (the reading-preferences
FAB, A05). `progress.rs` is the one submodule this step DOES own, ported separately (below) to
`catalog/progress.ts` because the library page needs it directly.

`web/src/lib/catalog/progress.ts` + its 10 tests port `progress.rs` completely: `parse`/
`serialize` (newline-set, `BTreeSet` → sorted array for a stable serialised form),
`completedCount`/`nextUnread` (against `readingOrder`, so the denominator can never disagree
with what the sidebar shows), `isAtEnd` with `END_THRESHOLD = 0.98` and both traps — `track <=
0` (a lesson shorter than the viewport; nothing to scroll is precisely "seen it all") and a
non-finite ratio (a page that hasn't laid out; `0/0` is not "at the end").

`web/src/lib/storage.ts` ports `storage.rs`'s three calls (`get`/`set`/`remove`) and the
swallow-failure contract (Safari private mode, a cookies-disabled profile — both throw rather
than return `null`, and a preference that can't save must never take the page down). The Rust
gets its "no window" case for free from `web_sys::window()?`; the SSR equivalent is an explicit
`typeof window === "undefined"` guard, because Astro's server render has no `window` at all.
Also spelled here: the full 8-key inventory (`reader-prefs`, `reader-progress`, `reader-last`,
`reader-sidebar`, `problem-pane`, `problem-approach`, `wb-language`, `theme`) as named exports,
so a typo in a future call site collides with a lint instead of silently starting a new key. A05
is the next consumer (documented in the module's own doc comment, not just here).

## The library page

`index.astro` replaces A01's placeholder. `client/src/catalog/view/library.rs` is the spec:
the hero pill, the CTAs, the category bands (`lib-group`) and book cards (`lib-card`) — byte-
faithful classes so `library.css`/`tour.css` apply unchanged. `BookCard.astro` is one component
covering both the Rust's `book_card` branches (a linked card when `firstLessonPath` resolves, a
dimmed non-link `div` when a book has no lessons at all) — the meta line
(`chapterCount`/`lessonCount`/`estimatedReadingMinutes`, "N chapters · M lessons · ~X min"),
≤3 tags, and the "Read →" CTA all come straight from `tree.ts`.

**Not ported this step, and it is a visible difference:** the `SynapseTour` carousel
(`view/tour.rs`, 440 lines — four auto-advancing slides, one of them a real `WidgetHost` playing
a hand-authored two-pointer trace). A static hero renders in its place, with an HTML comment
marking the spot. The carousel is pure UI chrome with no data dependency SSR needs to resolve —
nothing else in this step leans on it existing — so it can land as its own follow-up step
without disturbing anything built here. **Flagging for orchestrator sign-off, as instructed.**

### The progress chrome: what SSR can't do, and what runs after it

`localStorage` has no SSR equivalent, so the "N/M read" chip and the `.lib-continue` resume
card are absent from the server-rendered HTML — exactly like the oracle's own reactive
`progress_chip`/`ContinueCard`, which likewise render nothing until `ProgressStore` resolves;
the difference is *when* that resolution can happen, not *whether* nothing renders first.

`islands/library.ts` (vanilla TS, no framework — there is nothing here for Preact or Leptos to
hydrate INTO, only DOM Astro already rendered) does the rest post-load:

- **The island-props pattern chosen:** `index.astro` embeds the exact `SynapseIndexDto` it
  already fetched for SSR as `<script type="application/json" id="library-index-data">`
  (`<` escaped to `<` so a book title containing `</script` can't break the tag early).
  The island parses that blob once and calls the SAME pure `tree.ts`/`progress.ts` helpers the
  page just used server-side — no second network round trip, and no flatten/lookup logic
  written twice in two places. Each card carries `data-book-slug` (book slugs are globally
  unique — `findBook`'s own test says so), so the script re-resolves a book without re-walking
  the whole catalog tree per card; the continue card resolves through `bookOf`, same as the
  oracle.
- **(a) progress chips** — for each `[data-book-slug]` card, `findBook` + `completedCount` +
  `readingOrder().length`; a chip is inserted before `.lib-card__cta` only when `count > 0`,
  `--all` when the book is finished. Untouched books render exactly as SSR left them.
- **(b) the continue card** — `storage.get(READER_LAST_KEY)` → `bookOf` → the matching
  `readingOrder` entry; renders nothing until all three resolve, same rule as the oracle.
- **(c) "Start reading"** — the oracle's `scroll_to_grid` math (`grid.getBoundingClientRect().top
  + scrollY − 80`) ported verbatim into a click listener on `#lib-start-reading`.

## The blog

`web/src/pages/blog/index.astro` + `blog/[slug].astro` port `client/src/blog/view/mod.rs`
byte-faithfully: `blogList()`'s newest-first order is the server's, unchanged; the card fields
(eyebrow, title, summary, date, read time, ≤ unlimited tags — the oracle never capped blog
tags the way library cards cap at 3); the post's `prev`/`next` are publish-order neighbours
(`prev` = older, `next` = newer, per `BlogPostDto`'s own doc comment) rendered as the same
`← Older` / `Newer →` pager cards. The body crosses the identical `renderLesson` pipeline a
lesson uses (A03 moved it to `web/src/lib/markdown`) — the oracle never had a *second* markdown
path for blog posts, just the same DOM-injection call site with no workbench hydration
afterward. Unknown slugs 404 through the A03 pattern (`ApiFailure.status === 404` →
`Astro.response.status = 404` → the shared `NotFound` component).

**The leading-`<h1>`-strip had to change SHAPE, not behaviour.** The oracle's `loaded_post`
removes the pipeline's rendered `<h1>` via `body.query_selector("h1")` +
`previous_element_sibling().is_none()`, after the DOM exists. SSR has no DOM to query before the
response is written — `stripLeadingH1` does the same check as a string match on the rendered
HTML's leading tag (`/^<h1[^>]*>[\s\S]*?<\/h1>/` against the trimmed head of the string) instead
of a post-mount query. Same rule — a leading h1 has nothing before it by construction — enforced
at a different point in the pipeline because Astro's pipeline has no "after mount" moment for a
prose column that was HTML in the response to begin with.

**The head is new, not a port.** Neither blog page has an oracle string to match byte-for-byte:
the old client never called `seo::set_title` for anything but a lesson, so every blog page
under the SPA showed whatever generic title `index.html` carried, forever. This is the first
time the blog gets a real per-page `<title>`/description/canonical at all — an improvement
Astro's `output: "server"` gives for free by re-rendering the whole head as props on every
request, not a parity requirement this step had to hit.

## A small Astro-specific snag, and the fix

`index.astro`'s first draft filtered a category's nested entries down to books with an inline
type-predicate arrow function (`.filter((nested): nested is Extract<CatalogEntry, { kind:
"book" }>> => …)`). `astro build` failed with `Expected ")" but found ":"` — esbuild's JSX
transform for `.astro` frontmatter parses an arrow function's type-predicate return annotation
ambiguously against JSX tag syntax inside a template context. The fix pulled the filter into a
plain `booksOf()` function in the frontmatter script (ordinary TS, no JSX ambiguity to trip),
called from the template as `booksOf(catalogEntry.entries).map(...)`. Not a design decision —
a parser limitation worth naming so nobody rediscovers it by trial and error on the next page
that filters a discriminated union inline.

## Gates

- `dev-tools/check-conventions.sh` — clean; every new file well under the 800-line cap
  (`tree.test.ts` 258, the largest).
- `cargo fmt --all --check` · `cargo clippy --workspace --all-targets --all-features -- -D
  warnings` · `cargo test --workspace` — clean; this step touched no Rust.
- `(cd web && npm test && npm run build)` — vitest 60 → **88** (routes 2 + seo 2 + render 56 +
  tree **18** + progress **10**), `astro build` green.
- `(cd client && npm test && npm run build)` — vitest **27** unchanged (tracer 9 + stylesheets
  18); the release wasm + vite build succeeds, unchanged in shape from A03.

## Verified

```
cargo:  477 tests green (unchanged — no Rust surface this step)
web vitest: 88 tests, 5 files (routes 2, seo 2, render 56, tree 18, progress 10)
client vitest: 27 tests, 2 files, unchanged
build: web astro build ~700-860ms · client wasm:release + vite build green (render chunk
       543 KiB, unchanged — render.ts untouched this step)

e2e (astro front end: playwright → axum :8280 → astro_proxy → node sidecar :4321 → SSR → /api
     back to axum; real synapse-content, real Postgres synapse_rs)
  cd e2e && npx playwright test reader.spec.ts -g "head|sideways"
    ✓ the server renders a per-page head, not the placeholder
    ✓ the page does not scroll sideways
    2 passed

  NOT run this step: "the command palette opens and navigates" and "finishing a lesson is
  remembered across a reload" — both need chrome A04 does not build (⌘K is A05; the sidebar's
  done-tick class `.reader-sidebar__link--done` needs the interactive sidebar, also A05). The
  progress HALF those specs exercise (storage + the library's chip/continue-card) is proven
  below by hand instead of waiting on the rest of that spec to exist.

live, through the proxy on :8280 (real content, 7 books, 1 blog post):
  /              200, all 7 books + category bands render, hero pill + static CTAs, no tour
  /blog          200, 1 post listed, newest-first (trivially — only one post exists)
  /blog/{slug}   200, exactly ONE <h1> in the response (the header's — the pipeline's own
                 leading h1 confirmed stripped), description from the post summary, pager
                 empty (no neighbours — one post)
  /blog/nope     404, NotFound renders, title "Not found — Synapse"
  /synapse/...   200, unchanged from A03 (regression check)

  manual progress verification (localStorage seeded via a Playwright one-off against the real
  running proxy, then discarded — the full e2e spec can't cover this until A05):
    reader-progress = 2 of 4 Synapse Features lessons; reader-last = one of them
    → /  renders .lib-continue ("Pick up where you left off" / lesson title / book title)
       and .lib-card__progress "2/4 read" on the Synapse Features card
    "Start reading" click → scrollTo(0, 301.5) verified via an instrumented window.scrollTo
       (gridTop 381.5 + scrollY 0 − 80 = 301.5 — the oracle's exact offset math); the actual
       visible scroll could not be observed in this session's preview pane (innerHeight: 0 —
       the SAME documented limitation progress.rs's own doc comment names for the reason
       is_at_end lives in logic instead of the view layer); real-browser coverage of the
       scroll itself already exists via the e2e "does not scroll sideways" spec's layout
       assertions on the identical page.
```

## What deliberately does not work yet

- **The `SynapseTour` carousel** — flagged above; a static hero stands in for it.
- **Reading-preferences FAB, the ⌘K palette, the sidebar's filter/compact-rail/done-ticks** —
  none of these are this step's job; A05.
- **The problem workbench's `editorial.rs`/`pane.rs`** — noted in `tree.ts`'s own doc comment;
  A07.
- **Blog tags are uncapped** — matching the oracle's own `BlogListPage`, which never applied the
  library card's "≤3" rule to blog posts. Not a gap; a faithful difference.

## The lesson

**Paying a debt on schedule is cheaper than discovering it compounded.** A03 could have ported
`tree.ts`'s three walk helpers with an inline TODO and moved on; instead it wrote "A04 owns this
module and MUST add the 18 parity tests" into its own chapter, in the imperative, naming the
exact file and the exact count. That sentence is why this step opened by reading
`logic_tests.rs` completely before writing a single page — the ledger said what was owed and to
whom, and the debt turned out to be well-formed: eleven functions that didn't exist yet, zero
divergence in the three that did. A migration that tracks its own IOUs this explicitly never
has to reconstruct, three steps later, what "finish the catalog port" was supposed to have
meant.
