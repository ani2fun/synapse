# The cutover plan — synapse (Scala) → synapse-rs, one image swap

> Status: **PREP ONLY.** Nothing deploys until the pre-cutover fixes land and the go
> decision is explicit. The release workflow is `workflow_dispatch`-only until then.

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
- [ ] Sign-in round trip (PKCE, real realm) + `/api/me`.
- [ ] Run (python + java through the language tabs) → judged output.
- [ ] Submit: anonymous → 401 (enforced), allowlisted → 202 → poll → verdict.
- [ ] Visualise modal on a real trace; the widget gallery families.
- [ ] Practice widget: approach tabs, language-exact copy-to-editor, enlarge.
- [ ] Diagrams: mermaid, d2 slideshow, LikeC4 embed + fullscreen + click-to-guide.
- [ ] `/admin` as the prod admin; 403 as a non-admin.
- [ ] Mobile drawer at <1024px.
- [ ] `/media` images inside lessons; content JSON cache headers at the edge.
- [ ] CSP: sign-in unbroken, d2's ELK worker alive, wasm eval allowed
      (`'wasm-unsafe-eval'` — verify on the HEAVIEST pages under prod-shaped serving).
- [ ] git-sync: push a content change → visible within the sync interval, no restart.
- [ ] Lighthouse from a far region ≥ the Scala baseline.

## Rollback

`kubectl`-free by design: revert the kustomization commit in `ani2fun/infra` (the Scala
image tag) — ArgoCD converges back. The DB needs nothing: the baseline rows are inert to
Liquibase, and RS wrote no schema changes.

## Blocked on

1. The user's pre-cutover changes and bug fixes (list pending).
2. The `deploy/apps/synapse-rs/` overlay in `ani2fun/infra` (mirrors the synapse app:
   git-sync sidecar, sealed secrets, non-root securityContext — uid 65532 matches the
   image's USER).
3. The explicit go.
