# Design Q&A

Questions the user asks along the way, with the design reasoning — the synapse
convention, carried over.

## Q1 — Many Monaco editors on one page: what's the elegant shape? (2026-07-17)

**Question.** How are multiple runnable Monaco editors handled on a single lesson page?
Could one instance serve everything — e.g. plain ` ```python ` fences (no `run` attr) get an
"Open editor to try" button that opens a fullscreen popup codebench (Monaco + stdin + run +
console output, Esc to close, edit gated on sign-in) — so a page never over-loads with
editor instances and authors never need the `run` attribute just to make code tryable?

**What exists today (the facts the options rest on).**

- The Monaco *library* is already a lazy island chunk — it downloads and parses **once per
  page session** no matter how many editors mount. The per-block cost is the *instance*
  (`monaco.editor.create`): DOM, view zones, per-editor listeners — roughly 10–15 MB heap
  each. That is the thing worth bounding.
- Plain fences render through **shiki at parse time** — static HTML, zero runtime cost,
  no layout shift. They are the right reading surface and stay.
- A multi-variant `run` block already shares **one** Monaco across its language tabs
  (buffer + tokenizer swap in place — step 30); the practice widget lazy-mounts its
  editorial panes; the Visualise modal's editor mounts per open and disposes on close.
  So the swap-one-instance seam is proven.
- Current content has 1–3 `run` blocks per lesson — instance count is not yet a problem;
  the design is about not letting it *become* one, and about making every code block
  tryable.

**The options.**

- **A — the popup codebench (the user's proposal, refined).** One app-level
  `CodebenchStore` + one `<CodebenchModal>` mounted once in the shell (the proven
  `VisualiseModal` singleton pattern). Every shiki block hydrates a hover **"⤢ Open
  editor to try"** pill (the diagram-Enlarge `modal-btn` chrome); clicking opens the
  near-fullscreen modal with ONE Monaco created on first open and *reused forever after*
  (`setValue` + `setModelLanguage` — the step-30 seam), the fence's language pre-picked,
  Run + editable stdin + the runnable output panel (reusing `BlockStore.launch` and the
  existing panels), Esc to close like every other popup. Editing gates on auth exactly
  like inline blocks (the `wb__edit-bar` sign-in banner + the login redirect); Run stays
  anonymous-friendly. **Zero markdown changes** — authors write bare fences.
- **B — viewport-lazy `run` blocks.** Keep the authored inline-workbench UX, but a `run`
  fence renders its shiki + toolbar first and only creates Monaco when it scrolls near
  the viewport (IntersectionObserver, generous rootMargin; Run/Edit click force-mounts).
  Optionally an LRU that disposes editors far off-screen back to shiki — block state
  already lives in `BlockStore`, so nothing is lost. Bounds live instances to what's on
  screen (typically 1–2).
- **C — a true editor pool.** 2–3 `create`d editors total; every block owns only a cheap
  `ITextModel`; focus/scroll attaches a pooled editor to the active block
  (`editor.setModel` + DOM re-parenting). The most "IDE-like" and the most complex:
  re-parenting, focus restoration, per-block decorations and keybinding context all get
  subtle. Not warranted at 1–3 blocks per page.
- **D — click-to-mount.** Shiki until first interaction, then swap Monaco in for that
  block. Strictly dominated by B (same effect, worse first-interaction latency, no
  auto-readiness for the block you're looking at).

**Recommendation: A now, B when content density demands it, skip C/D.**

A is the high-value move: it adds a *capability* (every code block on the platform becomes
runnable) while structurally capping plain-fence cost at one shared instance, reuses five
proven pieces (modal singleton, language-swap seam, BlockStore, output panels, modal-btn
chrome), and needs no content migration. B is a contained follow-up that only pays off
once lessons carry many `run` blocks — measure first, since today's pages hold 1–3. The
Esc rule is already app-wide (viz modal, diagram zoom, practice enlarge, drawer); the
codebench simply joins it.
