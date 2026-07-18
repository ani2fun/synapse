# The cutover plan — synapse (Scala) → synapse-rs, one image swap

> Status: **EXECUTED 2026-07-18.** synapse.kakde.eu now serves this app. Kept as the record of how
> it was done and what it cost; the live ops truth is `infra/deploy/apps/synapse/README.md`.
>
> **What the plan got right.** The DB adoption rehearsal earned its place: baselining
> `_sqlx_migrations` on a byte-for-byte copy first proved that a verdict written by the Scala app
> (`completed`, 11/11) decodes through the Rust adapter unchanged — the one failure that would have
> been silent. Rollback stayed a one-commit revert throughout, and the in-place shape meant no DNS,
> certificate or ingress churn, and nothing left over to delete.
>
> **What the plan missed — all three found by touching production, not by testing around it.**
>
> 1. **`SYNAPSE_PORT` collided with a Kubernetes service link.** k8s injects Docker-link env for
>    every Service in the namespace, and the Service is named `synapse`, so `SYNAPSE_PORT` arrived
>    as `tcp://10.43.x.x:80`, overrode the image's `8080`, and the pod CrashLoopBackOffed. Caught
>    only because the rehearsal booted the real image **in the real namespace** instead of pointing
>    a local binary at a copied database. Fixed with `enableServiceLinks: false`.
> 2. **The release workflow had never once run.** It failed at startup with no jobs and no logs:
>    the repo's default workflow permission is `read`, and a called workflow can only narrow the
>    caller's token, so the engine's `packages: write` was an escalation. A `workflow_dispatch`-only
>    pipeline is an untested pipeline.
> 3. **No TLS backend was compiled in**, so sign-in 503'd on the live site while every test stayed
>    green — in dev every outbound call is plain http, which makes the production JWKS fetch the
>    system's only https caller. See `server/tests/outbound_tls_it.rs`.
>
> The through-line: each lived in the gap between "the tests pass" and "it runs where it will
> actually run". Two were caught by rehearsing in that place; the third was not, because the
> go/no-go list below exercises `/api/me` only anonymously — where 401 is correct either way.
> **A signed-in smoke check belongs in that list**, and is the single change that would have caught
> the one bug a real user hit.

## What cutover is

The infra shape does not change: same Deployment env, git-sync sidecar, realm, Postgres,
go-judge, likec4 service, Cloudflare rules. Cutover = point the Deployment at the
`ghcr.io/ani2fun/synapse-rs` image. The Scala image tag stays pinned in the kustomization
history for instant rollback.

## The DB adoption rehearsal (do this BEFORE any deploy)

The prod schema was created by the Scala app's **Liquibase**; synapse-rs runs **sqlx
migrate** at boot. Unrehearsed, sqlx would try to re-create existing tables. The rehearsal,
on a throwaway Postgres:

1. `pg_dump` the prod database; restore into a scratch container.
2. Diff the restored schema against what RS's migrations produce on a clean DB
   (`0001_*.sql`, `0002_*.sql` — submissions + allowlist). Expected: identical tables;
   Liquibase's own bookkeeping tables (`databasechangelog`, `databasechangeloglock`)
   are extra and harmless.
3. Baseline: insert the matching rows into `_sqlx_migrations` MANUALLY (version, checksum
   from `sqlx migrate info`) so sqlx considers 0001/0002 applied — the boot then no-ops.
4. Boot the RS image against the scratch DB; verify: boots clean, `/api/submissions/{id}`
   returns a REAL pre-existing submission (JSONB outcome decodes — the circe-wire parity
   goldens say it will), a new submit round-trips.
5. Only after 1–4 pass on the scratch copy does the same baseline run against prod, inside
   the cutover window.

## The go/no-go checklist (each item verified locally already, re-run against staging)

- [ ] OpenAPI snapshot green (`contract_it` — runs in every CI).
- [ ] Sign-in round trip (PKCE, real realm) + `/api/me`. **Do this AUTHENTICATED** — anonymous
      `/api/me` returns 401 whether the verifier works or not, which is how the no-TLS bug
      reached a real user. A junk bearer discriminates: 503 = JWKS unreachable, 401 = fetched
      and correctly rejected.
- [ ] Run (python + java through the language tabs) → judged output.
- [ ] Submit: anonymous → 401 (enforced), allowlisted → 202 → poll → verdict.
- [ ] Visualise modal on a real trace; the widget gallery families.
- [ ] Practice widget: approach tabs, language-exact copy-to-editor, enlarge.
- [ ] Diagrams: mermaid, d2 slideshow, LikeC4 embed + fullscreen + click-to-guide.
- [ ] `/admin` as the prod admin; 403 as a non-admin.
- [ ] Mobile drawer at <1024px.
- [x] `/media` images inside lessons; content JSON cache headers at the edge — Cloudflare cache
      rule applied 2026-07-18, `/api/synapse/*` + `/api/blog` reach `HIT`/`UPDATING` while every
      private route stays `DYNAMIC`. Setup + verification: the infra runbook, §6.
- [ ] CSP: sign-in unbroken, d2's ELK worker alive, wasm eval allowed
      (`'wasm-unsafe-eval'` — verify on the HEAVIEST pages under prod-shaped serving).
- [ ] git-sync: push a content change → visible within the sync interval, no restart.
- [ ] Lighthouse from a far region ≥ the Scala baseline.

## Rollback

`kubectl`-free by design: revert the kustomization commit in `ani2fun/infra` (the Scala
image tag) — ArgoCD converges back. The DB needs nothing: the baseline rows are inert to
Liquibase, and RS wrote no schema changes.

## Blocked on — all cleared 2026-07-18

1. ~~Pre-cutover changes and bug fixes~~ — shipped.
2. ~~The `deploy/apps/synapse-rs/` overlay~~ — **not needed.** The cutover went IN PLACE into the
   existing `deploy/apps/synapse/` unit, so there is no second overlay, no second Ingress, no
   certificate churn and nothing to delete afterwards. Promotes now patch that overlay.
3. ~~The explicit go~~ — given.

## What is left, and who owns it

- **Keycloak `first broker login` → Review Profile is still `REQUIRED`.** A first-time brokered
  user is shown an editable username field. Uniqueness already prevents claiming the existing
  `ani2fun`, so this is hardening rather than an open hole. Set execution
  `82b63ac3-b942-42b2-96d9-087f1b244423` to `DISABLED` (Admin Console → realm `synapse` →
  Authentication → Flows → first broker login).
- **The old Scala images stay in GHCR.** `ghcr.io/ani2fun/synapse:cde344a…` is the rollback
  target; do not prune until confidence is high. Pruning reclaims registry storage only — the
  cluster saving already landed (a ~6Mi Rust process replacing a JVM with a 256Mi floor).
- **The remaining go/no-go boxes above** need a signed-in browser session, which is the one thing
  the automated pass cannot do.
