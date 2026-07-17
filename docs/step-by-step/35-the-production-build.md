# Step 35 — The production build: one content-free image, gates inside

*(oracle: step 34 / ADR-S033 — the two-stage Dockerfile, origin compression, the media
cache hour, the GitOps workflows.)*

## What was still missing

Most of the oracle's step 34 had already landed in earlier RS steps — the content JSON
cache header (`max-age=60, stale-while-revalidate=600`, step 06), index `no-cache` vs
immutable hashed assets (step 18), the git-SHA-per-request content version with the dev
watermark (step 05), and the 700 KiB bundle-budget gate (step 02). Two origin concerns and
the entire image/GitOps layer were not:

- **`/media` didn't exist** — lesson images 404'd. `MediaRoutes` now serves the content
  checkout's `_media/` tree: traversal-guarded, explicit content types (SVG must be
  `image/svg+xml`), a single `bytes=` range answers 206 with `Content-Range`, and
  `Cache-Control: public, max-age=3600` rides on BOTH 200 and 206 — media is
  path-addressed but not content-hashed, so one shared hour. (Vite now proxies `/media`
  in dev.)
- **No origin compression.** `tower-http`'s `CompressionLayer` (gzip + deflate,
  `SizeAbove(1024)`) now wraps the whole router, OUTSIDE even the security-headers layer —
  compression at the origin on purpose: a CDN edge-compressing still pulls fat bytes
  across the tunnel. Sub-KiB responses stay identity. Four new ITs pin media types,
  the 206 contract, the traversal 404, and gzip-on-big/identity-on-small.

## The image

Two stages, CONTENT-FREE. The builder (`rust:1-bookworm` + Node 22 + binaryen +
`wasm-bindgen-cli` pinned to Cargo.lock's 0.2.126) builds the release server binary, then
`npm run build` chains the release wasm pipeline (cargo → wasm-bindgen → `wasm-opt -Oz`)
and `vite build` — and then **the bundle-budget gate runs as a build step**: an
over-budget bundle fails the image, not the reader's first paint. The runtime is
`debian:bookworm-slim` + CA roots (the outbound reqwest clients), the binary and the dist
under `/app`, `chmod -R a+rX` (the pod runs non-root — uid 65532 — and must read+traverse
everything while execute stays only where it was), and the prod ENV posture:
`STATIC_ROOT=/app/static`, `SYNAPSE_ROOT=/content` (the git-sync sidecar's volume),
**`SYNAPSE_AUTO_RELOAD=false`** (the oracle's env is `CONTENT_AUTO_RELOAD` — RS's config
prefix differs; the Dockerfile is the porting seam) → `contentVersion` = the checkout's
git HEAD SHA re-read per request, so a sidecar pull re-indexes with no restart.

## GitOps

Three workflow pieces, oracle-shaped: `_build-push-promote.yml` (the reusable engine —
buildx `linux/amd64`, ghcr tags `:<sha>` + `:latest`, gha layer cache, then patch the
`images:` entry in `ani2fun/infra`'s kustomization and push; a missing infra file warns
and exits 0 so CI can go live before the infra PR), `build-push-promote.yml` (the caller for
`ghcr.io/ani2fun/synapse-rs` — **manual-only for now**: no deploy until the pre-cutover
fixes land; the push-to-main trigger joins at the go decision), and a `docker` smoke-build job
appended to `ci.yml` (the image must always BUILD — the budget gate runs inside it).

## What the gate caught

The first in-image build FAILED its own budget gate — 838 KiB gz — and the failure was
real twice over. First: the LOCAL numbers had been stale for six steps. The budget script
measures `client/dist`; it doesn't rebuild it — every per-step "557 KiB ok" since step 27
was measuring an old dist while steps 28–34 grew the app (CI and the image, which rebuild
first, were always honest). Fresh local build: 839 KiB. Second: Debian bookworm's apt
binaryen is version 108 (2022) — pinned upstream binaryen 123 replaced it. The actual trim
came from a dedicated **`wasm-release` profile** (opt-level `z`, fat LTO, one codegen
unit, `panic = "abort"`, debuginfo stripped) — deliberately NOT the workspace release
profile, because `panic = "abort"` is right for a browser app and wrong for the server.
818 → 586 KiB gz wasm; critical path **606/700 KiB** on a fresh build.

## Verified

Native gates green (358 Rust — +6 media/compression — + 44 vitest); the media ITs pin the
cache hour on 200 and 206, the range slice, and the traversal guard against the real
router; the compression IT pins gzip-on-big / identity-on-small. The image (185 MB) built
with its internal gate at 606 KiB, then booted locally with the real content checkout
mounted at `/content`: Postgres connected + migrations applied at boot (fail-fast — the
first boot attempt without a reachable DB exited before binding, by design; prod supplies
the envs), `/api/health` answered, `/` served the SPA index, `/media/<file>` returned 200
with `public, max-age=3600`, and `/api/synapse/index` (6 entries from the mount) came back
gzip'd. **Not deployed**: the release workflow is `workflow_dispatch`-only — the
push-to-main trigger joins at cutover, when the pre-deploy fixes are in.

Next: the parity gate + cutover PREP — OpenAPI snapshot, the browser checklist — with the
deploy itself gated on the go decision.
