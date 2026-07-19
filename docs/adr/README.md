# ADRs — synapse-rs

Two kinds of decision govern this repository.

**Native decisions (RS…)** are made here, about this rebuild.

| # | Title | Status |
|---|---|---|
| [RS001](rs001-the-rust-rebuild.md) | The Rust rebuild: scope, stack, and discipline | accepted |
| [RS002](rs002-derivative-content.md) | Derivative study material never reaches the served catalog | accepted |

**Inherited decisions (S…)** were made in the oracle and still apply unchanged. Chapters cite
them directly rather than restating them — deliberately, since a restatement is a copy that can
drift.

The cost of that, until step 56, was that **105 citations across 19 distinct ADRs pointed at
nothing a reader here could open.** The oracle's `docs/adr-synapse/` is not vendored, and the
citations carry a number but no title, so following one meant already knowing what it said. For
a build book whose whole proposition is that the reasoning is followable, that is a real defect
— it just only bites a *second* reader, which is why it survived fifty-five steps.

They are not vendored now either, and that is the choice rather than an omission: a copy of
thirty-three ADRs would drift from the oracle silently, and the oracle is the authority. What
follows is an index — every S-number this repository actually cites, with its real title taken
verbatim from the source, and how often it is leaned on.

**Source:** `~/Development/homelab/synapse/docs/adr-synapse/README.md` — one file, `## ADR-S0NN — Title`
headings. Search the heading text; there are no per-file anchors to link to.

| # | Title (verbatim from the oracle) | Cited |
|---|---|---|
| ADR-S007 | Pragmatic hexagonal + light DDD | 8 |
| ADR-S009 | Friendly, leveled, colored logging (one format, both sides) | 7 |
| ADR-S010 | Content directory structure (the `synapse-content` layout) | 17 |
| ADR-S012 | Code-first wire contract for the catalog (hand-authored, not OpenAPI-generated) | 4 |
| ADR-S014 | Client architecture (three-layer rule, frontroute + a `Page` ADT, feature packages) | 6 |
| ADR-S015 | Markdown render pipeline (TypeScript, lazy, trusted HTML, shiki) | 9 |
| ADR-S018 | Adopt Cortex's design system (tokens, fonts, base), ported to Tailwind v4 | 2 |
| ADR-S019 | `openapi.yaml` mirrors the implemented REST surface (lean, grown per slice) | 4 |
| ADR-S021 | The `runCode` endpoint is code-first too (the OpenAPI codegen can't express it) | 1 |
| ADR-S024 | Identity: Keycloak as the IdP, stateless JWT validation at the edge | 1 |
| ADR-S025 | Tutoring: a scoped-down local coach, not a port of Cortex's tutor microservice | 5 |
| ADR-S026 | Visualization: a pure shared domain + a client feature, redesigned not ported (Phase 4) | 15 |
| ADR-S027 | The viz render contract + the one-shot content migration (step 26) | 2 |
| ADR-S028 | The declarative-widget spine: one host, one registry, layout-once (step 26) | 1 |
| ADR-S029 | The execution tracer: client-injected, harness-as-a-file, loud decode (step 28) | 5 |
| ADR-S030 | HeapToGraph: a staged pipeline, and deltas as a table (step 29) | 9 |
| ADR-S031 | The Visualise modal: an app-level player over a cached session (step 30) | 2 |
| ADR-S032 | Architecture-driven docs: a same-origin click-bridge + component-doc sidecars (step 32) | 2 |
| ADR-S033 | Production deployment: GitOps onto the Cortex cluster, a content sidecar, and speed at the origin (step 34) | 5 |

Thirty-three exist; the nineteen above are the ones this rebuild leans on. The remainder are
either Scala/Laminar-specific or superseded by an RS decision.

## What this directory is not

It is not a system reference. The 55 chapters in [`../step-by-step/`](../step-by-step/) are a
**narrative log** — they record what was built, in order, and why each decision looked right at
the time, including the ones that turned out wrong. They are excellent for "why is this like
this" and poor for "how does X work today", because a chapter is never revised after its step
except to fold in a fix to its own feature.

For current behaviour, read the code and its tests. For the reasoning behind a shape, find the
chapter that introduced it or the ADR it cites.
