# RS003 — The web tier is server-rendered Astro with TypeScript islands

**Status:** accepted · 2026-07-21

## Context

The reader shipped first as a Leptos CSR application: ~20k lines of Rust compiled to a 641 KiB gz
wasm bundle that had to boot before any prose appeared. Measured on production: content readable
at 1.25 s on broadband and **7.2 s on a mid-range phone over Fast-3G**, for lessons whose actual
content is ~2 KiB gz. The workload is ~99% reads of public, cacheable prose — the architecture of
the read path had to answer to that number.

## Decision

**The web tier is Astro SSR + TypeScript islands, served by a Node sidecar behind the axum front
door.** axum keeps everything it owned — `/api`, `/media`, `/c4`, robots/sitemap, security
headers, compression — and forwards page requests to the sidecar via `SYNAPSE_ASTRO_URL`
(mounted as the router fallback so registered routes can never be shadowed). Prose is HTML in the
response; interactivity hydrates as lazy, per-feature islands.

### Shape

| Concern | Choice |
|---|---|
| Pages | Astro 5, `output: "server"`, @astrojs/node standalone — SSR per request over the same public content API |
| Islands | Vanilla TS by default; Preact only where there is real component state (workbench, problem page, editorial, account/admin) |
| Cross-island seams | Named `CustomEvent`s + window providers, all declared once in `contracts.ts` — islands cannot share signals |
| Viz | The pure engine + renderers + Visualise modal stay Rust, as the standalone `viz-wasm` crate — a lazy 288 KiB gz bundle loaded only when a page has widgets or a viz-hinted workbench |
| Heavy libraries | Monaco, keycloak-js, mermaid, d2 — all lazy chunks behind loaders |
| Budgets | Per PAGE, not per bundle: gzip-sum of what each page kind's HTML loads eagerly, gated in CI against a live serve (250 KiB; measured 42/47/48/11) |
| Runtime | One container, two processes (`start.sh`, `wait -n` — either death kills the pod); `SYNAPSE_ASTRO_URL` empty → the server runs alone |
| Testing | Pure logic ported to vitest test-for-test; the Playwright suite is the view-parity harness and runs against the production-shaped serve |

### What deliberately did not change

The server's hexagon, the wire contract (the Astro tier consumes the same generated OpenAPI
types), the sandbox/judging/identity design, the content pipeline, and the stylesheets (ported
verbatim — the CSS was never framework-coupled).

## Consequences

- Content-readable falls from an app-bootstrap problem to a page-weight problem: tens of KiB of
  eager JS per page against the old 641 KiB wasm boot, with the regression class (an island going
  eager) caught by the CI budget gate.
- Two runtimes ship in one image (Rust binary + Node sidecar); the image grew 185 MB → 953 MB —
  the price of server rendering, paid knowingly.
- The Leptos client was deleted once the Astro serve passed the full e2e suite through the
  production-shaped stack; `viz-wasm` is the one Leptos surface that remains, by design (the viz
  engine's 93 tests + 16 cortex-goldens stay Rust, and the renderers were never worth porting).
- Rollback narrows from "env-flip to the old client" (which existed through the transition) to
  ordinary GitOps image reverts.
