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
