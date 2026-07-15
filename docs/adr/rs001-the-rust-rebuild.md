# RS001 — The Rust rebuild: scope, stack, and discipline

**Status:** accepted · 2026-07-15

## Context

Synapse (Scala 3/ZIO/tapir + Scala.js/Laminar, live at synapse.kakde.eu) is itself a deliberate
rebuild of Cortex, made slice-by-slice for understanding and ownership. `synapse-rs` applies the
same method one more time, with **Synapse as the reference oracle**: re-derive every slice cleanly
in Rust, port its test suites as the spec, never copy a decision you don't understand.

Motivations: learning + ownership · performance/footprint (no JVM on the homelab cluster) ·
Rust depth as a career skill · ecosystem consolidation (sbt+npm → cargo+npm).

## Decision

**Full-stack rebuild in a fresh repo.** Prod Synapse stays untouched until parity; cutover is a
single image swap (the Scala tag stays pinned for rollback). The Scala repo is frozen as the
oracle — no dual maintenance.

### Stack

| Concern | Choice |
|---|---|
| HTTP server | axum + tokio, tower middleware |
| Errors | `Result<A, DomainError>`, `thiserror` enum per context; `anyhow` only in the binary edge |
| Hexagon | ports = traits in `application/`, adapters in `infrastructure/`, DTO↔domain only at `http/`; `main` wires by constructor injection |
| API contract | code-first `utoipa` + shared serde DTO crate; a **contract-lock test** diffs the rendered spec against the committed oracle spec (`api/openapi.oracle.yaml`), which grows in lock-step as endpoints are ported |
| Persistence | sqlx (compile-time checked SQL) + `sqlx migrate` |
| Outbound HTTP | reqwest |
| Logging | `tracing` spans route→service→adapter (ADR-S009 parity) |
| Config | figment, env-first under the `SYNAPSE_` prefix (never the bare `PORT` — the preview-harness gotcha) |
| Shared kernel | one `synapse-shared` crate (wire DTOs + the viz engine), native **and** `wasm32` |
| Client | Leptos (CSR) — fine-grained signals, the Laminar-shaped choice; three-layer `logic/state/view` per feature |
| Client build | Vite + wasm-bindgen; the TS islands (render.ts, Monaco, mermaid/d2, tracers, keycloak-js) are reused verbatim |

### Discipline (enforced, not aspirational)

- **DDD:** bounded contexts as top-level modules; value objects as newtypes — never stringly-typed
  domain; ubiquitous language mirrors Synapse's CONTEXT.md.
- **Purity gates in CI** (`dev-tools/check-conventions.sh`, runs before the toolchain):
  server `domain/` uses no axum/tower/hyper/tokio/sqlx/reqwest/utoipa; client `logic/` uses no
  leptos/web-sys/wasm-bindgen/js-sys/gloo; file caps server/shared ≤ 500, client ≤ 800.
- **Anti-pattern lints as workspace law:** `forbid(unsafe_code)`; deny `unwrap_used`,
  `expect_used`, `panic` outside tests; clippy all+pedantic at `-D warnings` (curated allows are
  named in the root `Cargo.toml`); no `dyn` where nothing varies; no `async_trait` where native
  async-fn-in-traits works; no blocking in async; no `Arc<Mutex<_>>` pseudo-globals.
- **Testing:** integration tests from step 01 — every step drives the REAL assembled router
  (testcontainers Postgres, wiremock for go-judge/Keycloak/Ollama as they land; same `*_IT` env
  gates as the oracle); unit tests wherever there is logic; the viz engine ports against the
  cortex-goldens as native cargo tests.
- **Build book:** one chapter + one squashed, tagged `step-NN` commit per step; every tag compiles
  and its tests pass; chapters present the final design.

## Consequences

- tapir's single-source endpoint definitions have no Rust twin: the shared DTO crate + the
  contract-lock test replace them. Drift from the Scala contract is a red test.
- The client port is a re-derivation, not a transliteration — Laminar `Var`→Leptos `RwSignal`,
  `Signal`→`Memo`; the pure `logic/` layer stays native-testable.
- Unchanged and out of scope: go-judge, Keycloak (+ realm, `synapse-admin` client), Postgres,
  LikeC4 + the `/c4` proxy contract, synapse-content + git-sync, the infra/GitOps layout.
