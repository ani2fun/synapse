# A12 — The switchover (risk 3)

The step where every parity debt comes due at once: the pinned specs run against the real
proxy/CSP/compression path, a page kind that CI had never opened gets its spec, and the budget
gate is rewritten for a world where "the bundle" no longer exists. The A05 milestone did its
job — the seven specs had been passing against Astro locally for seven steps, and the
switchover itself changed none of them.

## The serve

`dev-tools/e2e` now starts the ASTRO topology by default — the Node sidecar on :4321, the real
axum server fronting it via `SYNAPSE_ASTRO_URL` — the same two processes the production image
will run (A13). It builds the viz wasm and web dist when absent, polls BOTH halves ready (the
health endpoint proves axum; a proxied page-fetch proves the sidecar), keeps the pipe hygiene
the 39-minute-hang lesson bought, and on failure dumps both logs. The old serve did not die:
`E2E_LEGACY=1 dev-tools/e2e reader.spec.ts mobile.spec.ts` runs the identical suite over the
old client — REHEARSED, 7/7 — because a rollback path nobody has run is a hope, not a path.
(The problem spec is excluded there by argument: the old client's problem page has floating
pills where A07 built the docked nav — the one honest selector difference.)

## The problem spec

`e2e/tests/problem.spec.ts` + a fixture problem (`learn/smoke/problems/threshold` — frontmatter,
run fence, testcases fence, editorial sidecar, all in the real authoring grammar). Three tests:
the SSR frame (crumbs VISIBLE below the header — the exact regression this session shipped and
fixed — tabs, docked nav, workbench extracted with case chips, zero page scroll), the editorial
stepper mounting on first tab open, and the Contents drawer being *genuinely visible* (it once
mounted into `display:none`). Run/Submit/auth stay out — go-judge and Keycloak are not this
suite's stack. The fixtures' pageerror guard rides along, so any hydration crash on the page
fails the spec even where no assertion looks. 7 → 10 specs.

## The budget

`check-page-budget.sh` replaces the single-bundle sum with the honest per-page measure: fetch
each page kind, collect what its HTML makes the browser download eagerly (module scripts +
stylesheets + the document itself), gzip-sum. Lazy islands stay out BY CONSTRUCTION — a dynamic
import never appears in the HTML — which retires the step-08 glob hazard for good. Measured:
landing 42 · lesson 47 · problem 48 · blog 11 KiB gz, against the old client's 700 critical
path. Budget 250 KiB (~5× headroom): if a page approaches it, an island went eager — tighten,
don't raise. The viz wasm gets its own cap (350 KiB gz, measured 281) checked only on release
builds (`VIZ_WASM_RELEASE=1`) — a dev-profile pkg failing for being unoptimized is noise. The
gate runs INSIDE `dev-tools/e2e` after the specs, while the stack is still up — no second
orchestration, and local runs get it free.

## CI

The e2e job builds the viz wasm RELEASE (the optimized artifact is what the specs exercise and
the budget caps — also the only place the binaryen-miscompile class can surface for it) and the
Astro app, and no longer builds the old client at all. `client-build` degrades to compile-only:
dev-profile wasm + a Vite build (which is what proves the repointed @aliases resolve) + the
vitest suites; the wasm-release/wasm-opt/700-budget minutes retired with the production build.
The Dockerfile still runs the full old-client build + old gate — A13's problem, noted here so
it isn't mistaken for an oversight. Likewise the release job's `needs` don't include e2e yet;
that gate moves when A13 reworks the release.

## Numbers

10/10 e2e (7 ported + 3 problem) through the Astro stack · 7/7 legacy rehearsal · pages
42/47/48/11 vs budget 250 · viz wasm 281/350 · fixture grows its first problem.

## The lesson

**A switchover you can rehearse in both directions is a configuration change; anything else is
a leap.** Everything this step "changed" was already true — the specs passed against Astro at
A05, the stack shape existed since A01's proxy — so the work was making the default tell the
truth and keeping the old truth reachable behind one env var. The budget rewrite carries the
same idea: measure what a reader actually pays on each page, not what a build artifact happens
to weigh.
