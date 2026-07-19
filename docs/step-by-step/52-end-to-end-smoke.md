# Step 52 — The end-to-end smoke suite

*(seven tests, and the first thing they found was a panic that had been shipping for weeks.)*

## Why

Fifty-one chapters carry a "Verified" section, and every one of them was a human in a browser
tab. That worked, and its failures are a matter of record:

- The nav drawer sat **under** the fixed header from step 33 to step 42 — nine steps — because
  every verification pass was done on a desktop viewport where the drawer isn't used.
- The ⌘K palette rendered at the page's bottom-left for a whole release (step 40): an orphaned
  CSS declaration swallowed `.cmdk-scrim`'s `position: fixed`. It was *visible* the entire time.
- Step 46 shipped 161px of horizontal overflow to production and it took a real iPhone to notice.

Every one of those is a machine's job. The existing gates — conventions, clippy, 433 Rust tests,
74 vitest, the bundle budget — structurally cannot see a widget that stops mounting, because
nothing in them opens a browser.

## Against the production serve, not Vite

The suite drives the **real axum server over a built `client/dist`**, not the dev server. Two
reasons, and the second is a scar:

1. Vite serves `index.html` off disk. Step 50's per-page `<title>`, the meta tags, the cache
   headers, `/sitemap.xml` and `/robots.txt` only exist on the server path — testing against
   Vite would assert none of it.
2. Step 19's standing lesson: **dev never reproduces CSP breakage**, because Vite serves without
   the origin's security headers. A suite that only ran against Vite would be blind to the class
   of bug that has hurt this project most in production.

Content comes from `e2e/fixture-content`, never `../synapse-content`. CI has no content
checkout, and pinning a smoke suite to another repository means an edit *there* can turn *this*
one red. The specs read the first lesson out of `/sitemap.xml` rather than hard-coding a slug,
so they are content-agnostic either way.

## What it found on the first real run

Three of seven specs failed, and the failures were not test bugs.

`.lesson-body` contained `<p>rendering…</p>` — the markdown island never completed, because the
wasm had **panicked**:

```
panicked at reactive_graph-0.2.14/src/traits.rs:361:29:
Tried to access a reactive value that has already been disposed.
PAGEERROR: RuntimeError: unreachable
```

A panic in wasm is a dead app: the body stays on "rendering…" forever, and every island on the
page stops responding. The palette failure was the same cause wearing a different hat.

**Getting the symbol was the whole diagnosis.** The release wasm reports bare
`wasm-function[2784]` frames. Rebuilding the dist with the *dev* wasm profile — `build-wasm.sh
dev` then `vite build` — produced a 65 MB artifact that named the line:

> At `reader.rs:211:43`, you tried to access a reactive value which was defined at
> `reader.rs:197:48`, but it has already been disposed.

`body_ref` is a `NodeRef` created under `loaded_lesson`'s owner. It is read inside a
`spawn_local` that renders the markdown — and that async block routinely **outlives the render
that spawned it**. Navigate away, or re-render the lesson, and the owner is disposed while the
island is still working.

The code already anticipated this:

```rust
let Some(body) = body_ref.get_untracked() else { return; };
```

That `else { return }` is exactly right in intent — "this render is stale, do nothing". But
**`get_untracked()` panics on a disposed reactive value rather than returning `None`.** The
guard could never fire. `try_get_untracked` is what makes it true instead of aspirational, and
the same hazard existed twice more in the same block: `mounts.set_value` on the success path and
`html.set` on the error path, neither of which went through the guard.

## Why production never showed it

Production is genuinely clean — checked directly, body renders, zero console errors. The window
is narrow: the async render has to finish *after* the owner is disposed, which needs a re-render
or a navigation while the island is mid-flight. Prod's server is faster and its content is
warm-cached, so the island usually wins the race.

It is a real bug on main regardless. It was reachable, it was intermittent, and intermittent is
the kind that surfaces on someone else's slower machine rather than in a verification pass.

## Two wrong turns worth recording

**A bisect that proved nothing.** I removed the step-50 and step-51 blocks from `loaded_lesson`,
rebuilt, saw the panic persist, and concluded "not mine". That conclusion happened to be right
and was not earned — step 51 also touched `sidebar.rs`, which was still compiled in. A probe
showing `sidebar links: 4` is what exposed the gap.

**A fix for a cause that did not exist.** When the suite failed serially as well as in parallel,
I set `workers: 1` and wrote a confident comment about CPU contention starving the wasm render.
Serialising made it *worse*, which disproved it. The setting is reverted — leaving it with that
justification attached would have been worse than the flakiness, because the next person would
have believed the comment.

There was also a self-inflicted one: I first ran the server on **:8281** to dodge a port
conflict, which is not in the Keycloak dev realm's origin list (5273/5373/8180/8280). The
unregistered origin 403s the silent-SSO iframe and made the panic deterministic — a red herring
that cost real time, and the same trap step 39 recorded when a silent Vite port bump broke auth.

## What this deliberately does not do

**No sandbox paths.** Run and Visualise need go-judge, so they belong behind an `E2E_SANDBOX`
gate alongside `GOJUDGE_IT` and `POSTGRES_IT` rather than making every push pull a 2 GB image.

**No authenticated paths.** Sign-in needs a live Keycloak with registered origins; anonymous
covers the reading surface, which is what every reader sees.

**No visual regression.** Screenshot diffing on a hydration-driven app is a flake generator, and
a smoke suite that people learn to re-run protects nothing.

**Chromium only.** One engine, two viewports. Cross-browser matters when there is an audience to
be wrong for.

## Verified

Seven specs, three consecutive clean runs, ~7s of browser time:

```
the server renders a per-page head, not the placeholder .......... ok
the lesson body renders and its prose hydrates ................... ok
the page does not scroll sideways ................................ ok
the command palette opens and navigates .......................... ok
finishing a lesson is remembered across a reload ................. ok
[mobile] the reader fits the screen .............................. ok
[mobile] the nav drawer opens and its close button is clickable ... ok
```

Green against both the fixture and the real `synapse-content`. The head assertion runs on the
**raw response before any JS executes** — what a crawler actually sees, which a `page.title()`
check would have passed even before step 50. The drawer test asserts via `elementFromPoint` that
nothing covers the close button, which is precisely how the step-42 bug was eventually found by
hand. And the progress spec exercises step 51's scroll-to-complete, which the dev preview could
not reach at all (it reports `innerHeight: 0`).

### What it took to go green in the runner

Locally the suite passed immediately; in CI it failed every hydration-dependent spec with a
bare `element(s) not found`, and it took two follow-up fixes to find out why.

The first was that **the gate could not fail at all**. `run: dev-tools/e2e | tee` takes its exit
status from `tee`, which always succeeds, so Playwright's failure was discarded — the job
reported success with `4 failed, 3 passed` in its own log, and because the step never failed,
`if: failure()` never uploaded the traces either. `set -o pipefail`, and a guard that now
requires *both* "something ran" and "nothing failed"; the original matched `N passed` and was
perfectly satisfied by `3 passed`. That is step 48's lesson repeating two steps later, in my own
work.

With the gate honest, the diagnostics named the cause in one line:

```
pageerror: WebAssembly.Table.grow(): failed to grow table by 4
```

That reads like memory pressure, and I treated it as such: a runner has ~4 GB shared with the
Postgres service container, and two workers across two projects meant several Chromium instances
each instantiating a multi-megabyte wasm module at once. I set `workers: 1`.

**It failed again, identically, on one worker.** `Running 7 tests using 1 worker`, a single
browser, same error. The hypothesis was wrong, and the run that disproved it is what forced the
right question: production is Linux and works, CI is Linux and does not — so look at how the
wasm is *built*, not at what runs it.

| | binaryen | wasm runs? |
|---|---|---|
| Production (Docker) | **123**, pinned upstream | yes |
| Local (macOS) | 130 | yes |
| CI (`client-build` and `e2e`) | **108-1, from apt** | **no** |

`apt-get install binaryen` on an Ubuntu runner gives **binaryen 108, released in 2022**, and its
`wasm-opt -Oz` MISCOMPILES a module built by the 2026 toolchain. The output loads and then dies
growing its function table.

The Dockerfile has pinned upstream binaryen since step 35, and its comment gives the reason as
*size* — 108 leaves ~270 KiB gz on the table. Correctness turns out to be the larger reason, and
nobody knew, because **CI built that broken artifact from step 35 onward and never executed it**.
`client-build` compiles the wasm, measures the bundle budget against it, and throws it away. The
budget was being measured on a binary that could not run. The e2e suite is simply the first thing
that ever loaded CI's own output in a browser — which is, precisely, the gap it was added to
close.

Both CI jobs now install upstream binaryen, reading the version **from the Dockerfile** so prod
and CI cannot drift onto different optimisers.

There is an uncomfortable coincidence in the middle of this: `workers: 1` had been set earlier in
this step and then *reverted*, because the reason given for it — CPU contention — was disproved
when serialising made things worse. It was reinstated here for a memory theory that was also
wrong. The setting is harmless and stays, but it never fixed anything, and pretending otherwise
would leave a third false explanation attached to the same line.

The CI job gates the release: a broken reader now stops a deploy. It carries the same
prove-it-RAN guard as the Postgres and sandbox gates — an empty run reports green, and that is
the failure mode these gates exist to prevent — plus a Playwright browser cache and trace upload
on failure.

433 rust + 74 vitest + 7 e2e. Critical path 646/700 KiB gz, unchanged.

## The lesson

**A guard written with the wrong API is not a guard.** `let Some(body) = … else { return }` reads
as defensive code and passes review as defensive code; it had been sitting there since step 08
looking like the disposal case was handled. It never once ran, because the call it guards panics
instead of returning `None`. The suite did not find a missing check — it found a check that could
not fire, which is the same shape as step 48's paths filter that could never match. Both were
invisible until something actually exercised them.
