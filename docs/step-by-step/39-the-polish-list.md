# Step 39 — The ten-item polish list

*(a user-reported bug/parity sweep across the reader chrome, the diagram cards, the problem
page, the Visualise modal, and the dev loop — plus one latent reactive-ownership bug the
sweep flushed out.)*

## The reader chrome

When the sticky heading bar slides in it is a fixed full-width bar at `top: 65px` — it was
covering the sidebar's toprow and the floating expand pill (the collapsed-sidebar restore
control). The oracle's drop rules now ride along: while `.reader-sticky[data-on="true"]`
shows, the sidebar drops to `top: 112px` (max-height follows) and `.reader-expand` to
`top: 120px`, both easing back when the bar hides. The expand pill also sits at `z-index: 46`
— one above the bar — so no scroll position can hide it.

## The diagram cards

Two placeholder-era leftovers were still shipping: the step-06 dashed "mounts in a later
step" frames (`.workbench` / `.solution-block` / `.quiz-block`) and a second white card
around `.mermaid-block` / `.d2-block` — the latter double-framed every diagram and pushed
the Enlarge pill visibly off the corner. Both are gone: placeholders are now bare mount
hosts carrying only prose margin, exactly the oracle's final design. The Enlarge and Close
affordances wear the shared **`.modal-btn`** teal pill (oracle: dialog.css) — lucide
maximize/x icon + label, Enlarge at the card's true top-left (6px, 6px), Close top-left in
the overlay. D2 slideshows get a **fixed figure box** (`clamp(16rem, 70vh, 32rem)`): the
steps ‹ › no longer resize the card — detail lives behind the zoom, as everywhere else.

Removing the quiz placeholder exposed a real gap: `render.ts` has planted `.quiz-block`
cards since step 16 and the client never hydrated them — the dashed frame was masking dead
authored content. The oracle's **QuizCard** is now a thin flat feature
(`client/src/quiz/`): prompt, options, Check (the right answer tints green wherever it is,
a wrong pick red), Try again — all state page-local, ungraded prose furniture.

## The workbench's per-case verdicts

Running with Case 1 selected marked *other* cases wrong, and hopping chips re-labelled the
same stale output under each case's expected — because the output panel judged the LAST
run's stdout against whichever chip was *currently* active. Oracle semantics now: the run
pins the case it was **launched** for (`TestsState.ran_case`), the arriving result is
judged against that case only, and the verdict lands in a **sparse per-case map** — chips
show ✓/✗ only for cases actually run. Switching chips clears the stale output panel through
the FSM's new `clear_outcome()` verb (buffer + edit unlock survive; the bumped handle
stale-guards any reply still in flight — pinned in `executor_tests`).

## The problem page

Problem pages now take the whole viewport: no 1600px cap, no sidebar column (breadcrumbs
carry the way back), slim padding — the two panes earn every pixel. In the Editorial tab,
the `##` headings become a **second row of section pills** (Intuition · Approach · Solution
· Dry Run · Complexity Analysis · …): the rendered DOM is grouped into `.pwb-esec` wrappers
and switching toggles only CSS, so the solution viewers' Monaco state survives
(`automaticLayout` re-measures on reveal). Multi-language solutions collapsed from stacked
per-language blocks into ONE viewer behind the **same language dropdown as the editor
pane**; Copy-to-editor sends the active tab's source to its matching workbench tab. The
header row's time/space chips are gone — the editorial's own Complexity Analysis section
already states them.

## The Visualise modal — editable, and a latent bug

The source pane is now **editable** Monaco, and one live `(source, stdin)` pair feeds every
re-trace path — the bar's ↻, the `r` key, and the stdin panel — so what re-traces is
exactly what's on screen. The Failed card carries the stdin box + Re-trace too: a bad input
is fixable in place, never a dead end.

Verifying this flushed out the real reason "Re-trace with this input" seemed to ignore the
new stdin: `obtain_fresh` minted the new session's `RwSignal` **inside the click handler's
reactive scope** — the very scope `store.open` disposes when it swaps the modal — so the
fresh signal died instantly and reading it panicked the reactive graph (the modal froze on
the old trace). Sessions live in a global cache and must outlive every view: they are now
minted under a **detached root `Owner`** (`SESSION_OWNER`), immune to whichever scope asked
for the trace.

## The dev loop

The auth-boot 403 ("Error while checking login iframe") was a port collision: both dev
loops claimed :5273/:8180, vite silently bumped synapse-rs to :5274 — an origin the
Keycloak dev client had never heard of. synapse-rs now owns its own pair — **vite :5373
(`strictPort`, so it can never silently drift) / server :8280** — and the dev realm's
`synapse-web` client registers both apps' origins (realm file + the running realm, updated
via the admin REST API). The two dev loops now run side by side.

Suites after this step: **362 native · 44 vitest**; clippy native+wasm clean; conventions
green. Verified live at :5373 — the sticky drop, the teal pills at (6,6), quiz cards on the
data-foundations pages, per-case badges + output clearing on flip-characters, the 8-pill
editorial, the Java/Python solution dropdown with language-exact copy, and two consecutive
stdin re-traces (`[x, y, z]` → array 0 1 2, `[p, q]` → array 0 1) with no panic.

## Postscript — the bundle gate (same day)

CI's in-pipeline budget gate rejected the step: 712/700 KiB gz. The step's own cost was
honest (~23 KiB for quiz + pills + the modal work) — the real story was that steps 36–38
had quietly spent the headroom (606 → 689). Two fixes landed as the follow-up commit:

- **`--cfg erase_components`** in the release wasm build: Leptos's statically-typed view
  tree monomorphizes a copy of the render machinery per view shape, and that was ~15% of
  the ENTIRE gzipped wasm. Type-erased views trade it for dynamic dispatch — invisible
  next to DOM cost. 686 → 584 KiB gz; the critical path landed at **609/700**. The
  release-vs-dev split was smoke-tested end to end on the built dist (problem page, Run +
  case badge, editorial pills, solution dropdown, Visualise trace — zero console errors).
- **`synapse-shared`'s `openapi` feature**: utoipa becomes optional, the `ToSchema`
  derives compile only for the server (`features = ["openapi"]`). LTO was already
  eliminating most of it (~1 KiB), but the wasm client no longer *compiles* the OpenAPI
  half of the wire contract at all — the schema story is a server concern.
