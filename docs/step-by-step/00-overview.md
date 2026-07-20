# synapse-rs — the build book

A deliberate, from-scratch **Rust rebuild of Synapse** (which was itself a rebuild of Cortex),
made slice by slice with **Synapse as the reference oracle** at `~/Development/homelab/synapse`:
its chapters are the design narrative, its test suites are the spec, its live deployment
(synapse.kakde.eu) is the parity target. Re-derive cleanly; never copy a decision you don't
understand. Scope, stack, and discipline: [RS001](../adr/rs001-the-rust-rebuild.md).

One chapter per step; one squashed commit per step, tagged `step-NN`. Every tag compiles and its
tests pass. Chapters present the **final design** of their step — bug fixes live in the step that
introduced the feature, never as later afterthoughts.

## The step map

| Phase | Steps | What they build (oracle steps in parentheses) |
|---|---|---|
| RS-P1 skeleton | 01–02 | Hello axum + typed `/api/health` + the discipline toolchain (01–02) · Leptos + Vite + first TS-island round trip + bundle baseline |
| RS-P2 catalog | 03–06 | The reference hexagon walk: domain → `ContentRepository` → filesystem adapter → http (03–06) · the Leptos reader + markdown pipeline (07–08) |
| RS-P3 execution | 07–09 | CodeExecutor FSM (09) · go-judge adapter + `/api/run` (10) · Monaco + RunnableCodeBlock + auth-gated editing + keymap (11 + post-33 fixes) |
| RS-P4 reader | 10 | Browse, palette, full-width problem pages, collapsible categories, reading preferences (12–13 + post-33) |
| RS-P5 submissions | 11–12 | Aggregate + async judging + sqlx (14–15) · 202/poll + workbench + live-refetch + chrome (16, 23 + post-33) |
| RS-P6 identity+platform | 13–18 | JWKS verify w/ canonical usernames (17, 36) · blog/search/rate-limit/proxies (19) · security headers+CSP (36, 38) · least-privilege account admin (21, 37) · admin allowlist panel (35) · tutor (20) |
| RS-P7 viz | 19–22 | The viz engine vs the cortex-goldens (24–27) · widget spine + Visualise + tracers (28–31) · the bespoke widget gallery (33) |
| RS-P8 UX + docs | 23–26 | "Your Turn" practice widget · landing tour + hero · mobile navigation + LikeC4 chrome (38) · architecture docs + capstone (32) |
| RS-P9 prod | 27–28 | The production build (34) · parity gate + cutover |

## Commit ledger

| Step | Tag | Chapter | Landed |
|---|---|---|---|
| 01 | `step-01` | [Hello, synapse-rs](01-hello-synapse-rs.md) | 2026-07-15 |
| 02 | `step-02` | [Leptos and the island bridge](02-leptos-and-the-island-bridge.md) | 2026-07-15 |
| 03 | `step-03` | [The catalog domain](03-the-catalog-domain.md) | 2026-07-15 |
| 04 | `step-04` | [The catalog application](04-the-catalog-application.md) | 2026-07-15 |
| 05 | `step-05` | [The catalog infrastructure](05-the-catalog-infrastructure.md) | 2026-07-15 |
| 06 | `step-06` | [The catalog HTTP layer](06-the-catalog-http-layer.md) | 2026-07-15 |
| 07 | `step-07` | [The Leptos reader](07-the-leptos-reader.md) | 2026-07-15 |
| 08 | `step-08` | [The markdown pipeline](08-the-markdown-pipeline.md) | 2026-07-15 |
| 09 | `step-09` | [The execution domain](09-the-execution-domain.md) | 2026-07-15 |
| 10 | `step-10` | [The go-judge adapter](10-the-go-judge-adapter.md) | 2026-07-15 |
| 11 | `step-11` | [Monaco and the runnable block](11-monaco-and-the-runnable-block.md) | 2026-07-15 |
| 12 | `step-12` | [Reader parity](12-reader-parity.md) | 2026-07-16 |
| 13 | `step-13` | [The submission aggregate](13-the-submission-aggregate.md) | 2026-07-16 |
| 14 | `step-14` | [Submissions: Postgres + 202/poll](14-submissions-infrastructure-http.md) | 2026-07-16 |
| 15 | `step-15` | [The workbench submit path](15-the-workbench-submit-path.md) | 2026-07-16 |
| 16 | `step-16` | [Identity, server side](16-identity-server-side.md) | 2026-07-16 |
| 17 | `step-17` | [Identity, client side](17-identity-client-side.md) | 2026-07-16 |
| 18 | `step-18` | [Platform breadth](18-platform-breadth.md) | 2026-07-16 |
| 19 | `step-19` | [Security hardening](19-security-hardening.md) | 2026-07-16 |
| 20 | `step-20` | [Least-privilege account admin](20-least-privilege-account-admin.md) | 2026-07-16 |
| 21 | `step-21` | [The admin allowlist panel](21-admin-allowlist-panel.md) | 2026-07-16 |
| 22 | `step-22` | [The tutoring coach](22-the-tutoring-coach.md) | 2026-07-16 |
| 23 | `step-23` | [The viz contract spine](23-the-viz-contract-spine.md) | 2026-07-16 |
| 24 | `step-24` | [The geometry families](24-the-geometry-families.md) | 2026-07-16 |
| 25 | `step-25` | [Design system + dark mode](25-design-system-dark-mode.md) | 2026-07-16 |
| 26 | `step-26` | [The adapt pipeline](26-the-adapt-pipeline.md) | 2026-07-16 |
| 27 | `step-27` | [The widget spine](27-the-widget-spine.md) | 2026-07-16 |
| 28 | `step-28` | [Tracers + the Visualise modal](28-tracers-and-the-visualise-modal.md) | 2026-07-16 |
| 29 | `step-29` | [The bespoke widget gallery](29-the-bespoke-widget-gallery.md) | 2026-07-16 |
| 30 | `step-30` | [The practice widget](30-the-practice-widget.md) | 2026-07-16 |
| 31 | `step-31` | [The diagram slice](31-the-diagram-slice.md) | 2026-07-17 |
| 32 | `step-32` | [The landing tour](32-the-landing-tour.md) | 2026-07-17 |
| 33 | `step-33` | [The mobile drawer + LikeC4 chrome](33-mobile-drawer-and-c4-chrome.md) | 2026-07-17 |
| 34 | `step-34` | [C4 click-to-guide](34-c4-click-to-guide.md) | 2026-07-17 |
| 35 | `step-35` | [The production build](35-the-production-build.md) | 2026-07-17 |
| 36 | `step-36` | [The reader chrome](36-the-reader-chrome.md) | 2026-07-17 |
| 37 | `step-37` | [The problem page](37-the-problem-page.md) | 2026-07-17 |
| 38 | `step-38` | [Toolbar + modal parity](38-toolbar-and-modal-parity.md) | 2026-07-17 |
| 39 | `step-39` | [The ten-item polish list](39-the-polish-list.md) | 2026-07-17 |
| 40 | `step-40` | [The page budget](40-the-page-budget.md) | 2026-07-18 |
| 41 | `step-41` | [Code blocks: tab groups](41-code-block-tab-groups.md) | 2026-07-18 |
| 42 | `step-42` | [Getting out of a problem page](42-problem-page-navigation.md) | 2026-07-18 |
| 43 | `step-43` | [Floating problem-page navigation](43-floating-problem-navigation.md) | 2026-07-18 |
| 44 | `step-44` | [Automatic releases](44-automatic-releases.md) | 2026-07-18 |
| 45 | `step-45` | [Production readiness](45-production-readiness.md) | 2026-07-19 |
| 46 | `step-46` | [The mobile reader](46-the-mobile-reader.md) | 2026-07-19 |
| 47 | `step-47` | [Sticky panes and a sticky language](47-sticky-panes-and-language.md) | 2026-07-19 |
| 48 | `step-48` | [Docs-only commits stop shipping](48-docs-only-releases.md) | 2026-07-19 |
| 49 | `step-49` | [Readership](49-readership.md) | 2026-07-19 |
| 50 | `step-50` | [Discoverability](50-discoverability.md) | 2026-07-19 |
| 51 | `step-51` | [Reading progress](51-reading-progress.md) | 2026-07-19 |
| 52 | `step-52` | [The end-to-end smoke suite](52-end-to-end-smoke.md) | 2026-07-19 |
| 53 | `step-53` | [The dev loop's ports](53-dev-loop-ports.md) | 2026-07-19 |
| 54 | `step-54` | [The books that were never meant to ship](54-unpublishable-content.md) | 2026-07-19 |
| 55 | `step-55` | [The tracer wrap, and a comment in the wrong file](55-tracer-wrap-tests.md) | 2026-07-19 |
| 56 | `step-56` | [The ADR index](56-the-adr-index.md) | 2026-07-19 |
| 57 | `step-57` | [The Editorial tab redesign](57-editorial-redesign.md) | 2026-07-20 |
| 58 | `step-58` | [The hydration seam](58-hydration-seam.md) | 2026-07-20 |
| 59 | `step-59` | [Discipline made structural, and the interface trims](59-discipline-and-trims.md) | 2026-07-20 |
| 60 | `step-60` | [AppDeps learns the ports it already had](60-appdeps-generics.md) | 2026-07-20 |
| 61 | `step-61` | [The bearer skeleton, stated once](61-bearer-gate.md) | 2026-07-20 |
| 62 | `step-62` | [Mechanism and vocabulary part ways](62-frontmatter-mechanism.md) | 2026-07-20 |
| 63 | `step-63` | [The test-case sink](63-the-test-case-sink.md) | 2026-07-20 |
| 64 | `step-64` | [The problem navigation, docked](64-the-problem-nav-bar.md) | 2026-07-20 |

> **Step 40's shape.** Every step through 39 is one squashed commit. Step 40 is eleven
> (`c9b302f..HEAD`): its work was interactive — a design question, two answers built and kept,
> a performance diagnosis, and the fixes that followed — and each commit was pushed to public
> `main` as it landed. Squashing after the fact would rewrite published history, so the tag
> simply marks the tip. The invariant that matters is untouched: `step-40` compiles and its
> tests pass.
