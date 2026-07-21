# synapse

An interactive platform for learning DSA / system design — prose chapters, runnable code, judged
practice problems, and step-through execution visualizations. Live at
[synapse.kakde.eu](https://synapse.kakde.eu).

- **Server** (`server/`): Rust — axum + tokio, hexagonal by bounded context. Owns `/api`,
  `/media`, the LikeC4 proxy, security headers, compression, robots/sitemap.
- **Web** (`web/`): Astro SSR + TypeScript islands, served by a Node sidecar behind the axum
  front door. Prose is HTML in the response; the editor, diagrams, search and the visualiser
  hydrate as lazy per-feature islands.
- **Viz** (`viz-wasm/`): the pure visualization engine + renderers + the Visualise modal, a
  standalone lazy wasm bundle (Rust/Leptos) consumed by the web tier.
- **Shared** (`shared/`): the wire DTO crate both server and viz build against.

Content is not in this repo: lessons are Markdown in a separate content repository, synced by a
git sidecar in production (`SYNAPSE_ROOT` points at the checkout). Decisions live in
[`docs/adr/`](docs/adr/); the scaling plan in [`docs/architecture/`](docs/architecture/).

## Run

```sh
dev-tools/dev          # axum API on :8280 + Astro dev (HMR) on :5373
curl localhost:8280/api/health
```

Backing services (Postgres, go-judge, Keycloak, LikeC4) come from the content platform's
docker-compose; the database is the dedicated `synapse_rs` on :5532.

## Test & gates

```sh
cargo test --workspace                                        # unit + integration + contract lock
cargo clippy --workspace --all-targets -- -D warnings         # the anti-pattern gate
cargo fmt --all --check
dev-tools/check-conventions.sh                                # purity + file caps
cd web && npx vitest run                                      # the TS suites
dev-tools/e2e                                                 # Playwright vs the production-shaped serve + per-page budget
```
