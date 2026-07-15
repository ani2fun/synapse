# synapse-rs

A deliberate, from-scratch **Rust rebuild of [Synapse](https://github.com/ani2fun/synapse)** — an
interactive platform for learning DSA / system design (prose chapters + runnable code + execution
visualizations). Built slice by slice with Synapse as the reference oracle and its test suites as
the spec, the same way Synapse itself rebuilt Cortex.

- **Server:** axum + tokio, pragmatic hexagonal by bounded context
- **Shared kernel:** one crate of wire DTOs + the pure viz engine, compiled native and to WASM
- **Client:** Leptos (fine-grained signals) + the original TypeScript islands (Monaco, mermaid/d2, tracers)

The design narrative lives in the build book (`docs/step-by-step/`, one tagged step per chapter)
and the ADRs (`docs/adr/`).

## Run

```sh
dev-tools/dev          # server on :8180 (override: SYNAPSE_PORT)
curl localhost:8180/api/health
```

## Test & gates

```sh
cargo test --workspace                                        # unit + integration + contract lock
cargo clippy --workspace --all-targets -- -D warnings         # the anti-pattern gate
cargo fmt --all --check
dev-tools/check-conventions.sh                                # hexagon/layer purity + file caps
```
