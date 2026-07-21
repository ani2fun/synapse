# A07 — The problem page

*(the two-pane workbench, server-rendered: a `kind: problem` lesson stops falling back to prose
and becomes the thing it always meant to be.)*

## The shape of the port

The Rust `ProblemWorkbench` was one 650-line component holding the whole page: breadcrumbs, the
left pane's head and tab bar, the description with the FIRST workbench cut out of it, the right
pane's runnable block, the submissions feed, the splitter, and the docked nav bar. The migration
splits that along the seam A03 already drew for lessons — **what the server renders vs. what the
island wires** — because the whole point of the Astro move is that the prose is HTML in the
response, not something a wasm bundle paints after it boots.

So the `.astro` page server-renders the entire static frame: crumbs (Home › book › lesson), the
title and lede, the three tab buttons + the difficulty badge, the DESCRIPTION markdown (through the
same `renderLesson` the reader uses), the hidden Editorial pane (the inline editorial, else the
`.editorial.md` sidecar the payload already carries), an empty Submissions host, the splitter, a
right-pane placeholder, and the nav bar with its "Problem N / M" counter computed at request time
from the book tree. `islands/problem` then extracts the first description workbench into the right
pane, hydrates the rest, and brings the tabs / splitter / submissions to life over the frame.

`pane.ts` ports `pane.rs` whole — the splitter width helpers **and** the label matcher (`pane.rs`
owns `section_index`; the module doc calls it "the label matcher the editorial shares"), with all
six of its parity tests camelCased. The step-65 record shrink is pinned: `problem-pane` now holds a
bare width, and the legacy `tab|pct|section` string is an explicitly-unreadable input that resets
the splitter exactly once. The editorial STEPPER that consumes `section_index` is A08's; this step
renders the editorial as plain markdown with the note that says so.

## Signals were already events

A06 turned the workbench's five threaded `RwSignal`s into named CustomEvents. A07 is the first
consumer of the far ends of that contract, and it needed **nothing new** on the workbench:

- The Submissions feed's "Use this test case" dispatches `synapse:use-case` ON the workbench root;
  the tests panel appends and selects it.
- "Copy to editor" on a revealed submission dispatches `synapse:load-code` ON the root; the
  matching language tab receives it (canonical, so a `python3` submission finds the `py` tab).
- A completed submit bubbles `synapse:submitted` up from the root to the document; the feed
  refetches on it.
- The one addition is the editor half of `synapse:relayout`, declared since A06 and consumed for
  the first time here: a pane that unhides a Monaco fires it so the editor re-measures.

The workbench root is a plain DOM element — the wrap div the right pane's Workbench renders into —
shared across panes through a module getter, because the Submissions feed (left pane) and the
Workbench (right pane) are the same island but different subtrees.

The Contents pill needed one new seam: `synapse:open-contents`. The problem page has no sidebar
column of its own (full-width panes, step 39), so its Contents pill fires the event and `reader.ts`
— already loaded for progress and prefs — opens the SAME drawer the mobile FAB drives, cloned from
a hidden `.reader-sidebar__inner` the page carries only as a clone source. One drawer, two triggers,
the event the seam between two islands.

## The trap: two conditional `<script>` blocks do not survive hoisting

The first cut wrote the obvious thing — a `{mode === "lesson" && <script>import workbench</script>}`
beside a `{mode === "problem" && <script>import problem</script>}`, each loading the right island
for its page. It built clean and it was wrong in the browser: the problem page ran the workbench
**auto-hydrator**, which claimed the first workbench in place, while the problem island never ran.
The right pane sat on "Loading the workbench…" and the description held a live editor it should
never have had.

Astro **hoists and bundles `<script>` tags statically**; the runtime `{cond && …}` only decides
whether a `<script src>` tag is emitted, and Rollup's code-splitting across the two entries handed
the surviving tag the wrong chunk. The browser was the only place this showed — the build was
green, the bundles existed, they were simply crossed.

The fix is one hoisted script with a **runtime** branch:

```astro
<script>
  import "../../islands/reader";
  if (document.querySelector(".pwb[data-problem]")) import("../../islands/problem");
  else import("../../islands/workbench");
</script>
```

One block, so there is no index to cross; a dynamic import, so a prose lesson never pulls the
problem code and a problem page never loads the auto-hydrator that would race its extraction. The
auto-hydrator also grew a belt to that suspenders — `if (!document.querySelector(".pwb[data-problem]"))`
around its `hydrateWorkbenches(document)` — so a stray import cannot start a second hydration. The
`.pwb[data-problem]` attribute is the single coordination point, and it is in the SSR HTML before
any script runs.

## What deliberately waits

**The Editorial stepper and approach tabs (A08)** — the pane renders plain editorial markdown, the
sidecar's `Intuition / Approach / Solution / Complexity Analysis` as prose. **Visualise (A10)** —
the workbench renders the button only once `window.__synapseViz` exists. **Real auth (A11)** —
until it installs `window.__synapseAuth`, Edit / Submit render disabled and the Submissions tab
shows the anonymous note, which is the correct anonymous experience, not a regression. **The Coach
tab (A09)** is not rendered — three tabs, not four, until the tutoring island lands.

## Verified

Gates: conventions · fmt · clippy (`--all-features -D warnings`) · cargo **479** (unchanged — no
Rust touched) · web vitest **144** (138 + pane's 6) · client 27 · both builds. Parity ledger: **70 of 101** ported (64 +
the problem-page slice: `pane.rs`, the two-pane view, the submissions feed, the docked nav bar,
the first-workbench extraction, the plain editorial). Seven e2e specs green (the fixture has no
problem lesson, so they never exercise this branch — but the shared `reader.ts` refactor and the
workbench guard had to leave them untouched, and they are).

Demo driven against a real judged problem on real content
(`/synapse/dsa/basic-problem-set-1/maths/reverse-a-number/reverse-a-number`, python + java, a
`.editorial.md` sidecar), through the axum → Astro-SSR topology (`SYNAPSE_ASTRO_URL`), the real
go-judge and the dedicated Postgres. The clean log flow, from a fresh tab:

```
ℹ️ problem page — /dsa/basic-problem-set-1/maths/reverse-a-number/reverse-a-number
🔍 hydrated 0 in-pane workbench(es), 3 fence group(s)     ← the first workbench was extracted
ℹ️ workbench mounted in the right pane (python/java)
```

Note the absence: no `hydrated 1 workbench(es)` from the auto-hydrator — the guard holds. Measured
in the DOM, not the console (console history doubles across a long-lived tab; the module fetched
once per the resource timeline):

```
two panes render            .pwb[data-problem], crumbs, EASY badge, Problem 3 / 13, 13 dot-links
right pane                  exactly ONE .runnable (python/java tabs) + the sign-in bar
description                 0 runnable, 0 workbench placeholders (first extracted), 3 fence groups
total .runnable on page     1                                    ← no double hydration
Submissions tab             "Sign in to see your submissions — they're private to you."
Editorial tab               Intuition · Approach · Solution · Complexity Analysis (sidecar)
Monaco across tab switches  survives (right pane never hides)
Run (anonymous)             Case 1 ✗ "Wrong answer", 0.010 s / 3 MB — the pass-stub, judged truly
splitter drag               46% → 33.72%, stored as bare "33.72" (not the legacy pipe)
reload                      width 33.72% restored · opens on Description · tab NOT persisted
Contents pill               drawer opens, 72 book links cloned in via synapse:open-contents
zero page errors
```

## The lesson

**A framework's static analysis does not see your runtime conditions, and the build being green
is not the same as the right code running.** Two conditional `<script>` blocks read as obviously
correct — each loads its page's island — and compiled to a page that ran the other one's. Nothing
short of opening the browser would have caught it; the migration's own rule (verify in a fresh
tab, against a production-shaped serve) is what turned "the right pane won't mount" into "the
auto-hydrator is claiming the first workbench," which is a fix. The single-script runtime branch
is not a workaround for a bug in Astro — it is the honest shape: there is one script tag, and it
decides at runtime, in the browser, where it is.

## Fixed forward (user bug report, 2026-07-21)

Two height-chain breaks surfaced together as "the test cases and output console are invisible."

**The shell lift.** The Astro problem page mounts `.pwb` directly under `.shell-main` (no
reader-layout wrapper), so the old client's `:has(.reader-layout …)` lift never fired and the
workbench sat inside a max-width, padded column. `shell.css` grew a twin keyed on the workbench
itself: `.shell-main:has(> .pwb:not(.pwb--embedded))` — embedded practice cards must not trigger it.

**The fill latch.** `.runnable__editor--fill` is `height: auto !important` (the container sizes
from monaco's root) while monaco (`automaticLayout`) sizes its root from the container — a
feedback loop whose fixed point is *wherever it starts*. One transient frame where the flex chain
is unbounded — the island hydrating before the pane height rule constrains it, a timing the old
client never hit because the wasm boot was slower than the stylesheet — and the editor latches at
5,228px forever, pushing the tests panel 4,000px down an invisible scroll. `contain: size` on the
fill container zeroes its intrinsic height, making the flex chain (min-height 220 + flex-grow) the
ONLY input; the latch cannot form and any stale layout self-heals on the next resize observation.
