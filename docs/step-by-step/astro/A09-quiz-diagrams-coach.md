# A09 — Quiz, diagrams, the C4 chrome, the coach, the codebench sheet

*(the placeholder families that were never anyone's single big risk — five small ports, one
relocated island, and a genuinely useful esbuild scar.)*

## The shape of the port

Six oracle files, six Preact islands, no unit-test debt (the parity ledger closed at A08 — this
step is view-only, verified live):

- **`client/src/quiz/mod.rs`** → `web/src/islands/widgets/Quiz.tsx`. The check-your-understanding
  card: select, Check tints right/wrong wherever they sit, Try again resets. Two hooks, nothing
  leaves the component.
- **`client/src/catalog/view/diagrams.rs`** → `web/src/islands/widgets/Diagrams.tsx`. Mermaid /
  d2 / d2-slideshow, each rendering its own task on the client at mount, the loud
  `.diagram-error` card with raw source on a malformed diagram, the zoom overlay (wheel zoom,
  drag pan, − % + ⟲, Enlarge/Close both top-left).
- **`client/src/catalog/view/c4.rs` + `c4_docs.rs`** → `web/src/islands/widgets/C4Embed.tsx` +
  `C4DocsPanel.tsx`. The LikeC4 iframe chrome (Enlarge → fullscreen zoom, synthetic ctrl+wheel
  pinch, the overlay guard, the scope-style injection) and the click-to-guide docs panel.
  `resolveC4Node` was already ported at A04 (`lib/catalog/tree.ts`) — this step is its first
  consumer.
- **`client/src/tutoring/mod.rs`** → `web/src/islands/coach/CoachPane.tsx`. The problem page's
  4th tab, mounted the same lazy way Editorial is.
- **`client/src/execution/view/codebench.rs`** → `web/src/islands/widgets/Codebench.tsx`. The
  one-Monaco-forever popup singleton; `fenceGroups.ts` grows the "Try in Editor" button it was
  missing since A06.

## The realm ceremony evaporates

`c4.rs`'s biggest cost was `js_sys::Reflect` everywhere: `wasm_bindgen::JsCast` casts
(`dyn_into`/`instanceof`) always fail across the parent/iframe realm boundary, so every property
read off a same-origin iframe's DOM went through `Reflect::get` instead of a normal accessor.
TypeScript has no such boundary — `el.tagName`, `frame.contentWindow`, `doc.querySelector(...)`
all work directly across frames, because this is plain JS running in the JS engine that also
owns the DOM it's touching. `C4Embed.tsx`'s `attachNodeBridge` and the overlay-guard
`MutationObserver` are half the line count of their Rust originals for exactly this reason. The
ONE place a foreign realm still matters: the synthetic ctrl+wheel pinch is built from the
**iframe's own** `WheelEvent` constructor (`frame.contentWindow.WheelEvent`, not the parent's),
because react-flow's handling runs in that realm — same rule as the Rust, just without the
ceremony needed to express it in wasm.

## Two small unifications beyond strict parity

- **`runnableFence`** (`lib/execution/language.ts`) is now what `codebench.rs`'s own comment
  says it should be: a thin delegation to `canonicalLang`, the SAME alias table the language
  preference reads — not a second copy that can go stale behind the server (the exact bug the
  Rust chapter for step-40 called out). Pinned by two new vitest cases: every one of the 22
  aliases (11 languages × their spellings, mirroring `server::execution::domain::Language::
  aliases` exactly) resolves `runnableFence` true, and `bash`/`toml`/`plaintext`/blank resolve
  false — plus a bare count assertion (`22`) so a server-side alias addition that never reaches
  this table fails loudly instead of silently.
- **`MarkdownPane`** (`islands/practice/panes.tsx`, shared by the editorial stepper AND the
  practice widget) now hydrates diagrams in both callers. The oracle was asymmetric here —
  `editorial.rs`'s `markdown_fragment` called `hydrate_diagrams`, `practice.rs`'s `markdown_pane`
  never did — and since the TS port unified the two Rust functions into one shared component
  from A08 onward, preserving that asymmetry would have meant threading a flag through for no
  real reason. A diagram authored inside a practice statement now renders instead of sitting
  inert; quiz and c4 stay lesson-body-only in both worlds (the oracle never needed them anywhere
  else, and neither does real content).

Blog posts get the SAME whole-document quiz/diagram/c4 pass, which is new, not a port — the
oracle's blog view (`client/src/blog/view/mod.rs`) never called a single `hydrate_*`, because the
oracle's blog content never carried a diagram fence. This migration's blog posts cross the exact
same `renderLesson` pipeline a lesson does, so the placeholder markup is identical whether it
lands in `/synapse/...` or `/blog/...` — leaving it inert here would have been an oracle
coincidence promoted to a design decision.

## The move: `@diagram` follows `@markdown`'s A03 pattern

`git mv client/islands/diagram web/src/lib/islands/diagram`, then `client/vite.config.mts`'s
`@diagram` alias repoints across the workspace — identical to how A03 moved the markdown
pipeline. The physical relocation meant `mermaid` and `@terrastruct/d2` had to become direct
`web/package.json` dependencies too: Node/Vite module resolution walks UP from the importing
FILE's own location through ancestor `node_modules`, and `web/` is that ancestor now, not
`client/` (client and web are separate npm projects, each with its own `node_modules` — there is
no root workspace to hoist through). `client/package.json` keeps its own copies, now vestigial
but harmless; nothing there imports them anymore, only the wasm glue's `@diagram/loader` extern,
which resolves through the alias at build time exactly as before. Both builds proven green
(`client npm run build`, `web npm run build`).

## The bug the tooling found: an em-dash inside an inline `<script>` breaks Astro's dep scan

The first draft of the lesson page's hoisted script carried a normal prose comment — em-dash,
apostrophe, the house style — INSIDE the literal `<script>...</script>` text. `astro dev`'s
cold-start dependency pre-scan (esbuild) choked on it with `Unterminated string literal` at a
line/column that didn't correspond to anything in the actual script, because Astro extracts
inline script bodies as separate virtual modules and something in that extraction path
mis-measures a multi-byte UTF-8 character. Bisected by stripping the comment down to plain ASCII
— the failure disappeared. Every OTHER inline `<script>` in this codebase either carries no
comment or carries one entirely outside the tag (in the `.astro` template's own JS expression
context, which is unaffected). The fix is narrow and permanent: comments that live inside a
literal `<script>` tag's text stay plain ASCII; the house-style em-dash prose goes in the
surrounding frontmatter/template comments instead, which this chapter's own `[...path].astro`
edits now do.

## What deliberately waits

**Viz (A10)** — the workbench still renders Visualise only once `window.__synapseViz` exists;
this is the LAST placeholder family, and the tour lands with it. **Real auth (A11)** — Edit,
Submit, and the codebench's edit lock all still read anonymous until `window.__synapseAuth`
installs; the codebench's sign-in bar is text-only, matching the rest of the anonymous
experience rather than growing a button with nothing to click yet.

## Verified

Gates: conventions · fmt · clippy (`-D warnings`) · cargo **479** (unchanged — no Rust touched) ·
web vitest **175** (173 + 2 `runnableFence` pins) · client vitest **27** (unchanged) · both builds
(`client npm run build`, `web npm run build`) · e2e **7/7** green (`dev-tools/e2e`, fixture
content — unaffected, since these still exercise the OLD client's `client/dist`). All new/moved
web files ≤ 800 lines (largest: `Diagrams.tsx` 337).

Demo driven against REAL content through `:8280` (`SYNAPSE_ASTRO_URL` → the Astro dev server on
`:5373`, `SYNAPSE_ROOT` → the real `synapse-content` checkout, the dedicated Postgres, the real
go-judge, the real LikeC4 compose service — all already running for this session, nothing stood
up standalone):

```
java-basics            3 diagrams hydrated, 0 diagram-error cards, 3 Enlarge affordances
  zoom overlay          Close top-left (47,31); wheel/±/⟲ all move the % readout; Esc closes
                        (a synthetic keydown on document did NOT propagate to the overlay's
                        window listener — a REAL keypress did; noted, not a defect)
storage-engines         1 d2 slideshow, 3 slides, 1/3→2/3→3/3, figure box 736×504 UNCHANGED
                        across every step, 0 error cards
architecture-docs        1 LikeC4 iframe, react-flow booted (3 nodes), click sfUser → "User"
                        guide (Actor · Web browser, 628 chars), click sfClient → switches to
                        "Client" (stale-drop proven — the first fetch never overwrote the
                        second), ✕ closes; Enlarge → fullscreen, Close top-left (14,12), the
                        live % reads real react-flow transform scale, the + button's synthetic
                        ctrl+wheel visibly changed the viewport's own scale (0.482 → 0.601)
analytics-and-column-   4 quiz cards; picked wrong → tinted red + "Not quite — the answer is
  stores                 "…""; Try again → picked right → "Correct ✓"
pattern-05 (problem)    4 tabs in order (description/editorial/coach/submissions); Coach tab →
                        "The coach is off / This feature is coming soon." (TUTOR_ENABLED unset)
codebench sheet         "Try in Editor" on a java-basics fence bar → modal opens (lang pill
                        "Java"), Monaco mounts, Run → REAL go-judge round trip: Accepted,
                        0.578s, 56MB, stdout "Hello World"
blog post               widgets script loads (200 OK, all 6 modules), 0 placeholders (none in
                        this post), 0 console errors — the new-not-ported wiring costs nothing
                        when there is nothing to hydrate
```

Zero console errors on every page above. The log flow for the coach demo, followable end to end
(ADR-S009):

```
ℹ️ problem page — /dsa/logic-building-pattern/pattern-05/pattern-05
ℹ️ workbench mounted in the right pane (python/java)
🔍 coach pane mounted
ℹ️ problem tab → coach
🔍 GET /api/tutor/config
```

And the codebench's, proving the fence-bar button → modal → real sandbox path:

```
ℹ️ codebench: opening a java snippet
🔍 codebench monaco mounted (java)
ℹ️ running java block
🔍 POST /api/run
🔍 run done: Accepted
```

## The lesson

**A migration step's risk is not always where the diff is biggest.** Diagrams.tsx is the largest
new file this step and it was also the least eventful port — the Rust was already a clean
state machine over three async renderers, and Preact hooks are a near-literal transliteration of
Leptos signals for that shape. The genuine surprise was four lines of prose inside a `<script>`
tag taking down `astro dev`'s cold start with an error message that pointed nowhere near the
actual bug. Ported code earns scrutiny by construction — it has an oracle to diff against. New
glue code, even a single import statement with a comment, does not, and this is where an
afternoon can disappear into a stack trace that lies about its own line number. The fix took
thirty seconds once isolated; finding what to isolate took the bisection.

## Fixed forward (user parity sweep, 2026-07-21)

The fullscreen LikeC4 zoom dropped its bottom-centre − / % / + pill (user call): LikeC4's
react-flow viewer renders its own zoom column, so the synthetic-wheel controls that earn their
keep on the small inline embed were a duplicate on the fullscreen one — the poll now watches
only the overlay state. And the ✕ Close became the shared teal `modal-btn` pill with the lucide
✕ (the bare class was an unstyled stray next to every other overlay's Close — an inconsistency
inherited faithfully from the old client's c4.rs, now fixed on the Astro side).
