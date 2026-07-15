# Step 11 — Monaco and the runnable block

*(oracle: synapse step 11 as-built — `monaco.ts` + `MonacoEditor`, the `CodeExecutor` FSM,
`Workbench` reduced to lesson scope, `RunnableBlocks` discovery — plus the post-33 editor keymap
absorbed as final design; `CodeExecutorSpec` + `RunnableBlocksSpec` essentials ported)*

## The `@editor` island

`monaco.ts` copied verbatim — every bundle-shaping decision preserved: `edcore.main` (NOT
`editor.api` — the full VSCode contribution set), one base worker, ten Monarch grammars, the two
synapse themes, and the `addAction` keymap (**⌘⏎ Run · ⇧⌘⏎ Submit · ⌘E Toggle editing** — in the
context menu and F1 palette, discoverable not folklore; each wired only where its verb exists).
One extension: `setReadOnly` flips read-only in place so ⌘E doesn't lose cursor/undo state.
`loader.ts` keeps the lazy split with the FFI-friendly flat-args shape; the Rust `MountedEditor`
owns the JS closures — dropping it disposes the editor AND the callbacks.

## The FSM and the three layers

`execution/logic/executor.rs` — the `CodeExecutor` FSM verbatim: `RunState`/`EditMode`
orthogonal, the opaque monotonic `RunHandle`, and the staleness trick (no real HTTP cancel:
`started`/`cancel` bump the handle; a late `completed`/`failed` with a stale handle is a no-op).
Nine tests pin every transition, transitions are `#[must_use]`. `logic/blocks.rs` — the
placeholder decode contract (trim langs, drop blanks, malformed → skip-not-crash).
`state/BlockStore` — the FSM in a signal + `launch()` (guarded like the Run button) +
`toggle_edit` (lock-up reverts the buffer; the RESULT survives). `view/RunnableBlock` — toolbar
(eyebrow · language pill · Edit with the instant `data-tip` tooltip · Run), the editor at the
oracle's height rule, and the output panel (status badge, `%.3f s` / MB meta, stdout, collapsible
stderr/compile-output, error panel).

The identity step adds the auth gate on Edit (oracle: `canEditSignal = authed && unlocked`) and
the submissions step brings ⇧⌘⏎'s target — staged exactly as the oracle staged them.

## Hydration — and the bug hunt that ended at the oracle's own pattern

Rendered lessons carry `<div class="workbench" data-variants="…">`. The first two hydration
attempts raced Leptos's render effects: a raf callback that never fired, then a direct call that
ran before `inner_html` hit the DOM (`hydrate: 0 placeholders`). The fix was to stop fighting
the framework and port the oracle's `MarkdownView` pattern verbatim: **write the lesson HTML
into the DOM directly, then mount blocks in the same breath** — no signal, no race. Boxed
unmount handles keep the mounts alive; navigation/unmount drops them, tearing down the monaco
editors. (Also banked: leptos wants `data-tip=…`, not `attr:data-tip=…` — the latter renders a
literal `attr:` attribute.)

## Verified

124 Rust tests + 40 vitest; clippy `-D warnings`; purity/caps/fmt. **In-browser against real
content + the real go-judge**: the flip-characters lesson hydrates one runnable block (Python
pill, Monaco mounted), Run round-trips through `POST /api/run` to the sandbox — the authored
stdin-reading starter correctly comes back `Runtime Error` with the real traceback, 0.025 s /
5 MB (its test-case panel is step 13's scope); the Edit toggle flips monaco in place with the
instant tooltip; zero console errors. Bundle: monaco is an 842 KiB gz LAZY chunk — the critical
path moved 226 → **266 KiB gz** (the wasm grew; still 2.6× headroom).
