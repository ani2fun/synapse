# A08 — The editorial and the practice widget

*(the parity ledger closes: the Editorial tab becomes a stepper, the practice widget lands, and the
reader/workbench/problem slice is 101 of 101.)*

> Numbering note: the "of 101" ledger counts the oracle client's parity units for the
> reader · workbench · problem · practice · editorial slice — the pre-viz/auth/coach scope. It
> started at A06 (64), grew at A07 (70), and closes here. viz (A10), auth (A11) and coach (A09) are
> separate later tracks and were never in this count.

## The shape of the port

Two oracle files each had a pure half and a view half, and the migration kept the seam:

- `client/src/catalog/logic/editorial.rs` (493 lines, 22 tests) → `web/src/lib/catalog/editorial.ts`
  + `editorial.test.ts`. The editorial document model: one markdown string in (the sidecar or the
  inline `<details>` tail — they share a shape), a typed `EditorialDoc` out. The flat format
  (`## Intuition / ## Approach / ## Solution / ## Complexity Analysis`) is one approach; the
  multi-approach format nests the same set as `###` under one `##` per approach and is DETECTED,
  never declared. Fence-aware splitting, spoiler-wrapper stripping, solution SYNTHESIS when the
  fences carry no Solution heading, complexity-claim extraction (paren-balancing, so
  `O(log(min(N1, N2)))` survives), and the `prettyO` display prettifier (`sqrt→√`, `*→·`, `^2→²`).

- `client/src/execution/logic/practice.rs` (262 lines, 7 tests) → `web/src/lib/execution/practice.ts`
  + `practice.test.ts`. The `.practice-problem` attribute decode into a `PracticeSpec`, approach-tab
  labelling (`approach-brute-force-1` → "Brute Force", numbered only when the kind repeats), and
  `solutionComplexities` — the fence-meta claim parser both `practice.ts` and `editorial.ts` share
  (it lives here because the oracle's `execution::logic::practice` owns it).

The two view halves — `catalog/view/editorial.rs` (585 lines) and `execution/view/practice.rs`
(473) — became four Preact islands under `web/src/islands/practice/`:

- **`EditorialPane.tsx`** — the approach stepper (numbered circles over a connector rail,
  per-approach `prettyO` complexities), the single-approach `★` bar, a sticky Jump bar with a
  rAF scroll-spy, numbered sections, and the Complexity section rendered as Time/Space cards when
  its prose parses.
- **`PracticeProblem.tsx`** — the `.pwb--embedded` two-pane card: PRACTICE badge, Description +
  approach tabs (lazy-mounted), a 9px splitter (28–64%), the reused workbench (Run only), and the
  ⤢ Enlarge toggle that CSS-promotes the SAME live panes to a modal.
- **`SolutionViewer.tsx`** — the read-only Monaco + language dropdown shared by both, with
  copy-to-editor.
- **`panes.tsx`** — the shared `MarkdownPane` (renders a fragment through `renderLesson`, then
  hydrates its `.solution-block`s and fence-groups) plus the `GatedSolution` reveal card.

## The one structural decision: the server ships markdown, not HTML

A07 server-rendered the whole editorial into `.pwb-editorial` and the pane showed it as prose. The
stepper cannot spend rendered HTML — it splits the editorial into approaches and numbered sections
and renders each section's markdown INDEPENDENTLY (so an approach re-visit is a cheap keyed swap,
and the gated solutions collapse again). So the `.astro` page now carries the RAW editorial
markdown on `data-editorial` and drops the SSR render entirely; `islands/problem` mounts
`<EditorialPane>` into the host on first open of the tab, parsing and rendering per-section in the
browser. This is exactly what the old Leptos reader did (it rendered client-side too), so it is
parity, not regression — and the Editorial tab is hidden behind a tab click, never on the critical
first paint.

The editorial pane host deliberately has **no `.pwb__pane-scroll` wrapper** (unlike the Description
and Submissions panes): the stepper renders its own `.pwb-epane > .pwb__pane-scroll pwb-escroll`,
mirroring the oracle's `ProblemWorkbench`, where the editorial pane is the one that does not carry
the shared scroll shell.

## Signals were events, and A08 is their last consumer

A06 turned the workbench's `load_code` RwSignal into `synapse:load-code` dispatched ON the workbench
root. A07 wired the Submissions feed's "Copy to editor" to it. A08 is the last consumer, and again
it needed **nothing new** on the workbench:

- The problem-page editorial's revealed solutions dispatch `synapse:load-code` on the right-pane
  workbench root (reached through the `workbenchRoot` getter `problem.tsx` already exposes to the
  Submissions feed). The workbench's own listener does the language-exact tab landing — a `java`
  solution finds the `java` tab.
- The practice widget mounts its OWN workbench imperatively (as `problem.tsx` mounts the right pane)
  and hands that root to its editorial's solution viewers, so a copy inside a practice widget lands
  in that widget's own editor.

Where the old client threaded a `(tick, lang, code)` triple, the event IS the tick — re-dispatching
fires again by construction.

## The scroll-spy lesson, carried across the port

Step 57's chapter pinned it and this port honours it: the Jump bar scrolls **the pane, not the
window**. `scrollIntoView` walks every scrollable ancestor and crept the page ~64px per jump;
`scrollSectionIntoView` measures the section against the pane and scrolls the pane directly, falling
back to the window only below the 1024px breakpoint where the pane stops scrolling and the page
carries the content. The 84px spy threshold and the 70px jump offset stay a pair.

## What deliberately waits

**Visualise (A10)** — the workbench renders the button only once `window.__synapseViz` exists.
**Real auth (A11)** — until it installs `window.__synapseAuth`, Edit/Submit render disabled and the
Submissions tab shows the anonymous note. **The Coach tab (A09)** is still not rendered. Diagrams
and viz widgets INSIDE an editorial fragment land inert until their steps (the `MarkdownPane`
hydrates solutions and fence-groups, the A08-relevant placeholders).

## Verified

Gates: conventions · fmt · clippy (`--all-features -D warnings`) · cargo **479** (unchanged — no
Rust touched) · web vitest **173** (144 + editorial 22 + practice 7) · client 27 · both builds. All
new web files ≤ 800 lines (largest: `editorial.ts` 443, `EditorialPane.tsx` 293).

**Parity ledger: 101 of 101.** The trail: A06 **64** (37 + executor 10 + blocks 9 + language 8) →
A07 **70** (+ the problem-page slice: pane, two-pane view, submissions feed, docked nav bar,
first-workbench extraction, plain editorial) → A08 **101** (+ the editorial model + stepper and the
practice decode + widget + solution viewer — the 29 oracle cases mirrored here plus their two view
components). The reader · workbench · problem · practice · editorial parity set is closed.

Seven e2e specs green (unchanged — the fixture has no problem or practice lesson, so the reader
regression suite never exercises this branch, and it stayed green through the web-only changes).

Demo driven against REAL content through the axum → Astro-SSR topology (`SYNAPSE_ASTRO_URL`),
DEDICATED Postgres, real content root, via a throwaway probe under `e2e/` (deleted after). The
problem `/synapse/dsa/logic-building-pattern/pattern-05/pattern-05` (python + java, `.editorial.md`
sidecar):

```
right pane opens on        Python
Editorial tab              single-approach ★ bar, 4 numbered sections, 4 Jump pills, 2 complexity pills
reveal → solution          read-only Monaco with a Java/Python dropdown
switch solution → Java, Copy to editor
right pane now on          Java                       ← landingProof: true
```

The log flow (SYNAPSE badge, ADR-S009), which IS the copy-to-editor tab-landing proof end to end —
this closes A06's untested multi-variant switch path:

```
ℹ️ problem page — /dsa/logic-building-pattern/pattern-05/pattern-05
ℹ️ workbench mounted in the right pane (python/java)
🔍 editorial stepper mounted (2569 chars)
ℹ️ problem tab → editorial
🔍 solution copied toward the java tab      ← SolutionViewer dispatches synapse:load-code
🔍 solution copied into the java tab        ← the workbench listener receives it
🔍 workbench tab → java                      ← language-exact landing
```

And the practice lesson `/synapse/low-level-design/oop/java-basics` (3 embedded widgets):

```
ℹ️ hydrated 0 workbench(es), 26 fence group(s)
ℹ️ hydrated 3 practice widget(s)
ℹ️ practice widget "Classes and Objects" — workbench (java)
ℹ️ practice widget "Attributes and Methods" — workbench (java)
ℹ️ practice widget "Constructors" — workbench (java)
```

Measured in the DOM: 3 `.pwb--embedded` cards, each with the PRACTICE badge, a workbench, a
splitter, an editorial tab that opens, and a working ⤢ Enlarge. Zero page errors on either page
(the probe's fixture fails on any uncaught page error; both passed clean).

## The lesson

**The last consumer of a contract is where you find out whether it was really a contract.** A06
declared `synapse:load-code` and A07 wired one end; A08 wired the two remaining ends — the
problem-page editorial and the embedded practice widget, each with a different workbench root — and
neither needed a single change to the workbench. The event carried the language, the workbench's
own listener did the canonical tab match, and a Java solution landed in the Java tab across a pane
boundary the old client had never actually exercised end to end. A seam that survives its second
and third consumers unchanged is an interface; A08 is where the workbench's stopped being an
assumption.

## Fixed forward (user bug report, 2026-07-21)

"The Jump pills don't navigate" — the pane never clipped, so there was nothing to scroll. The SSR
frame provides host divs (`.pwb-editorial-host`, `.pwb-coach-host`, `.psub-host`) for the Preact
panes to render into; a bare `display: block` div in the middle of a flex column refuses to shrink
below its content (`min-height: auto`), so the editorial spilled 300px past the pane invisibly and
`.pwb-escroll` measured `scrollHeight == clientHeight`. The step-57 scar said "scroll the pane,
never the window" — the pane also has to be ABLE to scroll. practice.css now relays the chain
through every island host (`flex: 1 1 auto; min-height: 0; flex column`); the rule is inert for
the old client, which renders its panes directly. Verified: content 754 in a 443 scroller,
Solution → 281, Complexity Analysis → 311, window pinned at 0.
