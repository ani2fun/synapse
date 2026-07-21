# A06 — The workbench

*(the migration's risk-1 step: 700 lines of view wiring rebuilt over a 43-test net that did not exist this morning.)*

## The shape of the port

The Rust workbench was one component carrying everything: per-variant stores over one Monaco,
the run/judge/staleness discipline, submit-and-poll, the tests panel with its case sink, the
verdict panel, viewport-lazy mounting with a page-wide cap, and five signal seams threaded from
the reader. The port splits along the seam the Rust itself documented — pure FSM vs view — and
the pure half went first, with parity:

- `executor.ts` (10 tests) — `RunState`, the branded monotonic `RunHandle`, and the staleness
  rule: a transition carrying a stale handle is a no-op.
- `blocks.ts` (9) + `language.ts` (8) — variant decode, `canVisualise`, display names, the
  11-language alias table.
- `judge.ts` + **16 shared vectors** (`shared/test-vectors/judge-vectors.json`): `judge` and
  `stdin_for` exist twice now — the server keeps the Rust, the workbench runs the TS — so both
  sides load the SAME vector file in their test suites, and drift between the twins is
  mechanically caught on whichever side moves. The vectors cover the edges that matter: status
  checked before stdout (one vector has coincidentally-matching stdout under a `RuntimeError`),
  `expected: ""` vs no expected, trailing-newline equality, out-of-order stdin assembly.

Two divergences, both documented in-source: `RunHandle` is a branded number (TS has no
module-private field to make it truly opaque), and the language table is exported (no
file-private visibility for a sibling test file to reach through).

## Signals become events

The Rust threaded five `RwSignal`s through props. Islands cannot share signals, so every seam is
now a named CustomEvent or window-scoped provider, declared once in `contracts.ts`:

| Old signal | New contract |
|---|---|
| `load_code: (tick, lang, code)` | `synapse:load-code` ON the root — the event IS the tick |
| `code_sink` | `synapse:code-changed`, bubbling — A09's coach snapshots it |
| `submitted` | `synapse:submitted`, bubbling — A07's tab refetches on it |
| `use_case: (tick, case)` | `synapse:use-case` ON the root |
| `stores.auth` | `window.__synapseAuth` + `synapse:auth-changed` (A11 installs) |

Until A11 installs the auth provider, Edit and Submit render disabled with the sign-in copy —
which is exactly the anonymous experience, not a regression. Visualise renders only once
`window.__synapseViz` exists (A10), rather than shipping a dead button.

## What the live demo caught

Driven against the real sandbox on a real judged problem (`count-all-digits-of-a-number`,
3 authored cases):

```
before interaction   shiki preview: 1   monaco: 0     ← viewport-lazy holds
Run                  badge: "Wrong answer ✗"          ← the starter stub, judged server-truthfully
                     chip 1 badged ✗                  ← verdict recorded against the LAUNCHED case
double-click Run     second click: disabled           ← staleness guard
switch chip          output cleared, badge stays      ← the step-39 semantics
after interaction    monaco: 1, page errors: 0
```

The first run of that probe found a real bug: **Monaco never mounted, with a `toLowerCase`
TypeError.** `mountEditor` in the moved loader is flat-positional — nine arguments, the
wasm-bindgen-friendly FFI shape the OLD client needs — and the Preact island called it with the
options object `createEditor` takes. In TS-land the shim is vestigial: the island now imports
`createEditor` directly (still dynamically, so Monaco stays a lazy chunk) and the loader remains
for the old client's externs until A14.

## What this deliberately does not do

**No practice widgets, no problem two-pane** — the component takes `practice`/`fill` props and
they are exercised in A07/A08, where their pages land. **No Try-in-Editor on fence bars** — the
codebench singleton is A09's; the bar ships tabs + copy rather than a dead button. **Tab
buffer-preservation is pinned by the FSM tests and the switch path, not by the live probe** —
the real content lacks a committed multi-variant lesson to drive (the java+python singleton
lesson remains uncommitted in synapse-content); A08's copy-to-editor demo exercises the same
`switchTo` path end-to-end.

## Verified

Gates: conventions · fmt · clippy · cargo **479** (477 + 2 vector tests) · web vitest
**138** (95 + 43) · client 27 · both builds. Parity ledger: **64 of 101** ported
(37 + executor 10 + blocks 9 + language 8). Live demo as above, zero page errors.

## The lesson

**A seam that exists for one consumer is an assumption, not an interface.** `mountEditor`'s
nine positional arguments were the right shape for wasm-bindgen and the wrong shape for
everyone else, and nothing said so until a second consumer arrived. The fix was not to fix the
shim but to stop using it — the native `createEditor` options object was sitting underneath the
whole time. Migrations surface these because they are second consumers of everything.

## Fixed forward (user parity sweep, 2026-07-21)

**viz.css was never imported by the Astro app** — the workbench's language dropdown
(`.wb__lang-menu`, step 38's chrome) rendered as bare stacked buttons on every page, A06 through
A10, because the classes were right and the stylesheet was absent. It rides [...path].astro and
index.astro now (the tour's slide-4 widget needed it too). And the long Submit why-copy tooltip
wraps (`max-width: min(26rem, 70vw)`) instead of running one nowrap line off the pane's left
edge — `.pwb__right`'s `overflow-y: auto` computes overflow-x to auto as well, so anything wider
than the pane was clipped mid-sentence.
