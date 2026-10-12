# Architecture — synapse-rs

The full C4 description of this codebase — system context, containers, components and code, plus
deployment — is Part 2 of the **Synapse** book, served by the app itself at
[`/synapse/synapse-features/system-context/system-context`](https://synapse.kakde.eu/synapse/synapse-features/system-context/system-context)
and kept in [`ani2fun/synapse-content`](https://github.com/ani2fun/synapse-content) under
`synapse-features/09-system-context/` onwards. It carries the context → container → component
walkthrough, one page per bounded-context group, and the schema.

This page is the one-screen summary that stays next to the code. Diagrams are `d2` fences drawn
from the source beside them (ADR-RS009, ADR-RS010) — there is no separate model file and no viewer.
When a container is added or removed, update this diagram and the book's
[Containers](https://synapse.kakde.eu/synapse/synapse-features/containers/containers) page together.

## Containers

```d2
direction: right

browser: "Browser\n[Astro pages · Preact islands · viz-wasm]"
cdn: "Cloudflare\n[CDN]"

pod: "synapse pod (one image, two processes + a sidecar)" {
  origin: "origin\n[Rust · axum · :8080 prod / :8280 dev]"
  web: "web tier\n[Astro 7 SSR · Node 22]"
  d2: "d2-render\n[Go · :8390]"
}

sandbox: "go-judge\n[runner/go-judge]"
db: "PostgreSQL 17\n[10 tables]" { shape: cylinder }
files: "content checkouts\n[git-sync primary + satellite cache]" { shape: stored_data }
sync: "git-sync"
idp: "Keycloak\n[realm synapse]"
github: "GitHub"
coach: "Ollama\n[local only]" { style.stroke-dash: 3 }

browser -> cdn -> pod.origin
pod.origin -> pod.web: "page (fallback)"
pod.web -> pod.origin: "API"
pod.web -> pod.d2: "d2 fence"
pod.origin -> pod.d2: "walkthrough boards"
pod.origin -> sandbox: "POST /run"
pod.origin -> db
pod.origin -> files
pod.origin -> idp: "JWKS"
pod.origin -> github: "satellite tarballs · edit PRs"
pod.origin -> coach: "TUTOR_ENABLED only" { style.stroke-dash: 3 }
github -> sync -> files
```

## Inside the origin

Twelve bounded contexts in one binary (`server/src/lib.rs`), with 22 port traits between them and
their adapters: `catalog`, `blog`, `execution`, `submission`, `identity`, `authoring`, `progress`,
`notes`, `canvas`, `insights`, `tutoring`, and the flat cross-cutting `platform`. See
[`hexagonal-ddd-audit.md`](hexagonal-ddd-audit.md) for the layering audit and
[`scaling-plan.md`](scaling-plan.md) for what changes first under load.
