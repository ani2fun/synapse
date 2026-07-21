# ADRs

The decisions that shape this repository. One file per decision; a decision is revised by a new
ADR superseding it, never by silent edits.

| # | Title | Status |
|---|---|---|
| [RS001](rs001-the-rust-rebuild.md) | The Rust rebuild: scope, stack, and discipline | accepted |
| [RS002](rs002-derivative-content.md) | Derivative study material never reaches the served catalog | accepted |
| [RS003](rs003-the-astro-web-tier.md) | The web tier is server-rendered Astro with TypeScript islands | accepted |

For current behaviour, read the code and its tests — ADRs record the reasoning behind the shape,
not a reference manual. Operational architecture (deployment, the scaling ladder) lives in
[`../architecture/`](../architecture/).
