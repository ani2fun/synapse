# Production readiness — synapse-rs on the k3s homelab

> **Nothing here deploys anything.** The release workflow stays `workflow_dispatch`-only until
> the go decision is explicit. This is the ordered plan plus an audit of what is actually
> missing, done by reading the code rather than by checklist.
>
> Companion: [cutover-plan.md](cutover-plan.md) covers the DB adoption rehearsal, the
> go/no-go parity list, and rollback. This document covers **readiness of the service itself**.

## Part 1 — the deploy steps, in order

Each step is a gate: do not start the next until the previous is green.

1. **Fix the two blockers below** (graceful shutdown + the health endpoint). They are the only
   findings that change code; everything else is configuration or infrastructure.
2. **Rehearse the DB baseline** on a scratch restore of the prod dump — the full procedure is
   in `cutover-plan.md`. sqlx must consider `0001`/`0002` already applied over the
   Liquibase-created schema, or boot will try to re-create live tables.
3. **Author the infra overlay** `deploy/apps/synapse-rs/` in `ani2fun/infra`, mirroring the
   Scala app: Deployment + Service + the git-sync content sidecar, `runAsUser: 65532` with pod
   `fsGroup: 65532` (matching the image's `USER 65532:65532` and the git-sync volume),
   resource requests/limits, and probes (§Blocker 2 defines them).
4. **Seal the secrets** into the cluster: `SYNAPSE_DATABASE_URL`, and
   `KEYCLOAK_ADMIN_CLIENT_ID` / `KEYCLOAK_ADMIN_CLIENT_SECRET` for the scoped `synapse-admin`
   service-account client. Never in the manifest.
5. **Set the prod config flips** (§Medium 3) — the dev defaults are wrong for production in
   two security-relevant places.
6. **Build and promote one image** via the existing workflows, but deploy it to a
   *staging* host/namespace first, not the live one.
7. **Run the go/no-go parity list** from `cutover-plan.md` against staging, including the
   authenticated paths and a CSP check on the heaviest pages.
8. **Cut over**: point the Deployment at the `ghcr.io/ani2fun/synapse-rs` tag. The Scala image
   tag stays pinned in kustomization history — rollback is reverting that commit.
9. **Watch**: first rolling update is the real test of Blocker 1's fix. Submit something during
   the roll and confirm it reaches a terminal state.

## Part 2 — the audit

### Blocker 1 — no graceful shutdown, and it orphans submissions

`server/src/main.rs` ends with a bare `axum::serve(listener, app).await?` — there is no
`.with_graceful_shutdown(…)` and no SIGTERM handler anywhere in the server. On any pod
eviction, rolling update, or node drain, Kubernetes sends SIGTERM and the process dies at once.

This is not merely "some 502s during a deploy". Judging is **asynchronous**: `POST` writes the
row as `SubmissionState::Judging`, returns 202, and a detached tokio task completes it later.
The application layer has a `JudgeFailed` backstop so a *judge error* never leaves a row stuck
— but that backstop runs *inside the task*. Kill the process and it never runs, and there is
**no boot-time reconciler** (verified: nothing sweeps orphaned rows). So the row stays
`Judging` **forever**, and the client polls it until it gives up.

Every promote is a rolling update, so this fires on every single deploy that lands while
someone is submitting.

Fix — two parts, both small:

- Wrap the serve in a shutdown signal, and give the runtime a drain window:
  `axum::serve(...).with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })`
  extended to SIGTERM via `tokio::signal::unix::signal(SignalKind::terminate())`. Set the
  Deployment's `terminationGracePeriodSeconds` above the judge's worst case.
- Add a **startup reconciler**: on boot, sweep rows left in `Judging` older than the judge
  timeout and complete them as `JudgeFailed`. This is the durable fix — it also covers OOM
  kills and node failures, which no shutdown hook can catch.

Ship the reconciler even if the shutdown hook lands: the hook reduces the window, the
reconciler closes it.

### Blocker 2 — the health endpoint is still the walking-skeleton stub

`server/src/platform/health.rs` returns a constant:

```rust
HealthStatus { status: "ok (walking skeleton)".to_owned() }
```

Two problems. First, that literal string becomes the public production health response —
`/api/health` is reachable through the SPA routing. Second, and materially: it checks nothing.
Wired as a **readiness** probe, a pod whose Postgres is unreachable still reports ready and
keeps receiving traffic it cannot serve.

Fix — split the two probes, because they want opposite things:

- **Liveness** = the current shallow check. Shallow is *correct* here: a liveness probe that
  fails on a DB blip makes k8s restart a healthy process and turns a dependency outage into a
  crash loop. Just change the string.
- **Readiness** = a new endpoint that pings the pool (`SELECT 1`) and reports degraded.
  Postgres is the only hard dependency — the server already fail-fasts at boot without it;
  go-judge and Ollama are degradable and must **not** gate readiness.

Note the interaction with boot: the server fail-fasts if Postgres is absent at startup, so the
overlay also needs a `startupProbe` with enough failure budget, or a DB blip during rollout
turns into a crash loop.

### Medium 1 — the rate limiter is per-process

`platform/rate_limiter.rs` holds `Mutex<HashMap<…>>` in-process. Memory is fine (it prunes
above a threshold). The correctness caveat is scaling: with N replicas the effective limit is
**N × configured**, and every deploy resets all windows.

At one replica this is exactly correct, which is the homelab's shape. Decide explicitly:
either pin `replicas: 1` and write down why, or move the counter to Postgres/Redis before
scaling out. The danger is scaling to 2 later and silently doubling the limit.

### Medium 2 — no metrics

Observability is `tracing` to stdout. There is no `/metrics`, no Prometheus registry. For a
single-user homelab that is a defensible choice, but it means the only way to answer "is it
slow?" is logs. If the cluster already runs Prometheus, a `metrics-exporter-prometheus` layer
is a small add; if not, defer deliberately rather than by omission.

### Medium 3 — prod config flips that are wrong by default

Two of these are security-relevant, and both default *open* for dev convenience:

| Variable | Dev default | Production |
|---|---|---|
| `SUBMISSION_ALLOWLIST_ENFORCED` | false (anonymous submit → 202) | **true** |
| `ADMIN_USERS` | `tester` | **`ani2fun`** |
| `OIDC_ISSUER` / `OIDC_AUDIENCE` | local :8181 realm | prod realm — also parameterises the CSP |
| `LIKEC4_URL` | local | `http://synapse-likec4/c4` — the proxy strips the prefix and the image serves under it; dropping `/c4` breaks embeds |
| `SYNAPSE_AUTO_RELOAD` | true | false (already baked into the image) |
| `KEYCLOAK_ADMIN_*` | dev client | sealed secret, scoped `synapse-admin` client only |

Shipping with the dev defaults would leave submissions open to anonymous users and grant admin
to a username that does not exist in the prod realm.

### Low

- **No `HEALTHCHECK` in the Dockerfile** — irrelevant under k8s, whose probes supersede it.
  Leave as is.
- **Runtime base is `debian:bookworm-slim`**, not distroless. It carries `ca-certificates`
  for the outbound reqwest clients, which is the reason. Fine; it just inherits Debian's patch
  cadence, so the image needs periodic rebuilds even when the code is unchanged.

## What is already sound

Worth stating, so the list above is read as short rather than as the whole picture: the image
is content-free and runs non-root with a bundle-budget gate inside the build; security headers
and an issuer-parameterised CSP ship on every route class with exact-value tests; the admin
path uses a scoped service-account client rather than realm admin; usernames are canonicalised
at the verifier; the OpenAPI contract is snapshot-tested in CI; and content versioning re-reads
the git SHA per request so the sidecar re-indexes with no redeploy.
