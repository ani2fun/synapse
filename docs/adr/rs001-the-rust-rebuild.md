# RS001 — The Rust rebuild: scope, stack, and discipline

**Status:** accepted · 2026-07-15 · **amended 2026-07-30** (see *Amendment* below)

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
| Shared kernel | one `synapse-shared` crate (the wire DTOs), native **and** `wasm32`. The viz engine lived here until step 45 and now sits in `client/src/viz/engine/` — the server never referenced it |
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

## Amendment — 2026-07-30

The **Errors** row above says `thiserror` enum per context and stops there, which left the
variants' PAYLOADS unruled. In practice the codebase had already discovered the rule twice and
broken it twice, so it is worth stating.

**An error payload is a `String` when only humans read it, and TYPED when anything branches on
it — and it is flattened at the EDGE, never before.**

`ContentError::IndexInvalid(SynapseContentError)` is the shape working: the merge failure travels
whole through the application layer, and `catalog::http::dto` is the one place it becomes a
sentence. Nothing upstream had to guess what the client could use.

The failure mode is `map_err(|e| MyError::Thing(e.to_string()))` written inside an application
layer. It reads as harmless — the text survives, after all — but it decides on the edge's behalf
that the edge has nothing more to say than that one sentence. `AuthoringError` did this to
`InvalidEdit` and the cost was concrete: four unrelated mistakes (empty file, oversized file,
frontmatter deleted, title gone) reached a contributor mid-edit as one "not valid", because by the
time the HTTP layer could offer a per-rule remedy the rule was gone. It now carries
`Rejected(InvalidEdit)` and each rule gets its own next step.

The rule cuts both ways, and `FetchError::RateLimited { seconds: u64 }` is the other edge of it.
The field is typed for a consumer that was never written: `ContentSync::fail` records every
variant identically, so a throttled source is refetched on the very next tick. Typing a payload
does not create the branch that justifies it — and a doc comment promising the branch is worse
than no comment, because it reads as a description instead of an intention.

The rule is about crossing a LAYER, not crossing a process. Flattening at `http/` is correct and
expected: that is the boundary where a client's vocabulary — a status code and two strings —
genuinely is the whole contract.
