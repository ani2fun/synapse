//! The catalog feature (oracle: `client/catalog/` — ADR-S014's grown three-layer split):
//! pure `logic/` (native-testable, purity-gated) → reactive `state/` → `view/`.

pub mod logic;
pub mod state;
pub mod view;
