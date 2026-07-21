# Scaling Synapse: from hundreds to millions of users

> **Status:** forward-looking plan (2026-07-15). Nothing here is committed work — each stage is gated
> by the triggers in its header, and the design intent is that **most stages never need to happen**
> ahead of real traffic. The companion narrative, written as a system-design case study, is the
> capstone lesson in the System Design from First Principles book
> (`synapse-content/system-design-from-first-principles/06-case-studies/14-synapse-capstone.md`).
> Current-state ops truth: `infra/deploy/apps/synapse/README.md`.
>
> Carried over from the Scala repo (now `ani2fun/synapse-scala`, archived) at the 2026-07-18
> cutover. It is implementation-agnostic — every stage is about traffic classes and triggers,
> not about the language — so it belongs with whichever implementation is live.

## The shape of the problem

Synapse's workload is three very different traffic classes, and the whole plan follows from keeping
them separate:

| Class | Traffic share | Character | Scaling lever |
|---|---|---|---|
| **Reads** (lessons, index, blog, media, pages) | ~99% | public, cacheable, version-addressed | the CDN, not the origin |
| **Runs** (`POST /api/run`) | ~1% | CPU-bound, interactive (sync), **untrusted code** | a sandbox fleet |
| **Writes** (submissions, allowlist, account) | «1% | small rows, async judging, per-user | Postgres, barely |

Napkin math for the **1M MAU** target (the numbers the stages are sized against):

- 1M MAU → ~100k DAU → **~10k concurrent** at the peak hour.
- Reads: a lesson every ~2 min per concurrent reader ≈ **~80 lesson loads/s peak**. A lesson JSON is
  ~40 KB; content is derived data keyed by a git SHA, so ≥95% of this is CDN-servable → the origin
  sees **single-digit requests/s**. Reads are a cache-correctness problem, not a compute problem.
- Runs: if ~2k of the 10k concurrent are actively coding and run every ~5 min ≈ **~7 runs/s peak**,
  ~1 CPU-second each → **~10–20 dedicated cores** at peak, 5× burst headroom → an autoscaled fleet
  of tens of cores. This is the only real compute bill in the system.
- Submissions: ~1 per 10 runs → **<1/s**. At 2 KB/row that's ~400 MB/day worst case — a partitioned
  Postgres holds years of this on one primary.
- Auth: sign-ins are rare (sessions are long, reading is anonymous); token *verification* is local
  JWKS-cached JWT checking (step 17/36) — **zero per-request auth cost** at any scale. Keycloak only
  ever has to survive login bursts.

**Conclusion the plan is built on:** scaling reads is delegated to the edge; the engineering effort
concentrates on the sandbox fleet (capacity *and* blast radius) and on operational maturity. The
modular monolith does not get broken up on a growth chart — a bounded context is only extracted when
its scaling behaviour measurably diverges (the `execution` context is the designated first candidate).

## What is already scale-ready (do not re-buy these)

Decisions already in the codebase that the plan leans on rather than revisits:

- **Stateless origin.** No server sessions — identity is a JWT verified against cached JWKS
  (ADR-S024); any request can land on any replica. Horizontal scaling is a replica count.
- **Content is derived, versioned data.** `contentVersion` = the content checkout's git HEAD SHA,
  re-read per request (ADR-S033); the git-sync sidecar re-indexes with no redeploy. Cache keys are
  therefore *correct by construction* — the missing piece at higher stages is only making the URLs
  version-addressed so TTLs can be long.
- **Edge caching in place.** Static assets are immutable-cached (Cloudflare HIT); public content
  JSON already ships `max-age=60, stale-while-revalidate=600`.
- **Async judging.** Submissions are 202 + poll (qna Q26) — the write path is already shaped like a
  queue consumer even though today the "queue" is in-process.
- **Rate limiting** (step 19) with the 429 → sign-in nudge UX, and the allowlist gate on
  submit-and-save (step 35) as an abuse valve.
- **Hexagonal contexts** (ADR-S007): the seams to extract a context into its own deployable are the
  package boundaries that already exist; the CI convention gate keeps them clean.

## Stage 0 — today (single node, hundreds of users)

k3s on one node: origin pod (pages + API + media, non-root) with the git-sync sidecar, go-judge pod,
Keycloak, Postgres, LikeC4 via the `/c4` proxy; Cloudflare in front; GitOps deploy loop
(push → CI → ghcr → promote → ArgoCD). Fine as-is; the only Stage-0 action items are the ones
already on the roadmap: the Cloudflare cache rule for `/api/synapse/*` + `/api/blog/*`, and a load
baseline (k6 scripts for the three traffic classes) so every later stage has a before/after.

## Stage 1 — first thousands (DAU ~1k–10k)

**Triggers:** sustained origin CPU > 60%, read p95 > 300 ms at the edge, run p95 > 3 s, or any
single-node availability incident that hurt.

1. **Multiple nodes, multiple replicas.** 3× origin replicas behind the Service + HPA on CPU. Each
   pod carries its own git-sync sidecar (a read-only clone is per-pod state that replicates
   trivially). PodDisruptionBudgets + anti-affinity across nodes.
2. **Postgres gets serious.** Move to a managed Postgres or an in-cluster operator (CNPG) with WAL
   archiving + PITR backups, and put **pgbouncer** in front (the replica count multiplies connection
   pools). Nightly restore drills, not just backups.
3. **Keycloak HA.** 2 replicas against the shared DB; sessions are Keycloak's problem, not ours
   (the app never holds one).
4. **go-judge becomes a pool.** N replicas with per-pod concurrency caps and CPU/memory quotas on a
   dedicated node pool (taint/toleration), so a run burst can never starve the origin. The server's
   `CodeRunner` port already round-robins through a Service.
5. **Observability as a deliverable.** Prometheus + Grafana + Loki (or managed equivalents), tracing
   on the run/submit paths, and **published SLOs**: read p95 ≤ 200 ms (edge), run p95 ≤ 2 s,
   availability 99.9% reads / 99.5% runs. Alerts page on SLO burn, not on raw CPU.

## Stage 2 — tens to hundreds of thousands

**Triggers:** origin read QPS > ~200/s despite the 60s TTL, run queueing observed at peak, DB
connections/replication pressure, or a real abuse incident.

1. **Version-addressed content → origin-less reads.** The web tier learns to fetch content at
   SHA-addressed URLs (`/api/synapse/<sha>/…`, the client gets the current SHA once per session);
   those responses become immutable (`max-age=1y`). The origin then serves each lesson **once per
   content push per region** — reads scale to any audience the CDN can hold. Media moves to object
   storage (R2) behind the same CDN.
2. **The judge fleet.** Split the run path onto its own deployable (the `execution` context's
   designated extraction): a thin run-gateway + an autoscaled go-judge fleet (scale on in-flight
   runs / queue depth), **warm pools** per language so p50 stays interactive. Submissions judging
   moves behind a real queue (the 202 + poll contract doesn't change — only the consumer does).
3. **Sandbox hardening to match exposure.** go-judge's namespaces/cgroups stay the inner wall; add
   an outer wall per pod — gVisor or Firecracker class isolation, `NetworkPolicy` egress-deny,
   seccomp profiles, and per-user run quotas with anomaly alerts. The blast-radius rule from step 37
   (least-privilege credentials everywhere) is the template: assume a sandbox escape and size what
   it can reach.
4. **Postgres: one read replica** (feeds, admin lists, tutor context reads) + statement timeouts +
   the partitioning DDL for `submissions` (by month) prepared and rehearsed.
5. **Load-shedding order, written down:** tutor first, then runs (429 with the existing calm UX),
   never reads. Degradation is a product decision made in advance, not an incident improvisation.

## Stage 3 — millions

**Triggers:** sustained six-figure DAU, a second geography where run p95 breaks SLO from RTT alone,
or a single-region availability posture that's no longer acceptable.

1. **Reads: multi-region by default.** Content is immutable + SHA-keyed by now, so this is CDN
   configuration plus (optionally) regional origin replicas for cache fill — no data problem exists;
   content has one writer (git) and infinitely many readers.
2. **Runs: regional judge fleets.** Route `POST /api/run` to the nearest fleet (latency-based
   routing); a run has no cross-region state at all. This is the piece users feel — interactive
   latency is the product.
3. **Writes: stay single-writer, get honest about it.** Submissions remain in one primary region
   (partitioned, archived to object storage past N months); cross-region users eat ~100–200 ms on
   submit — invisible inside an async 202 + poll flow. Only if write volume ever breaks the napkin
   math (it shouldn't: <1/s at 1M MAU) does user-homed sharding enter the conversation.
4. **Organizational scaling.** On-call rotation, chaos drills (kill a judge node at peak; lose a
   region's CDN), quarterly restore + failover exercises, a cost dashboard per traffic class
   (the run fleet is the only line item that grows superlinearly with engagement — watch it).
5. **The tutor** (ADR-S025) gets its own budget and back-pressure: LLM spend scales with engaged
   users, so per-user daily quotas + a queue with visible position, and it is the first thing shed.

## What deliberately does not change

- **The modular monolith** stays the unit of deployment for everything except the run path. The
  hexagon's contexts are the extraction seams *if and when* a context's scaling diverges — extraction
  is a response to measurement, never to fashion.
- **Postgres** remains the system of record throughout; nothing in the workload justifies a
  distributed database at any stage of this plan.
- **The content model** (git as the authoring plane, derived read models, SHA versioning) is the
  load-bearing idea at every stage — Stage 2's origin-less reads are just its logical conclusion.
- **Security posture ratchets, never relaxes:** every stage that adds capacity to the sandbox fleet
  must ship its matching isolation hardening in the same step.
