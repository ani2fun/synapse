//! The catalog's inbound HTTP adapter (oracle: `CatalogRoutes.scala`): axum routes → the
//! service, wire DTO ↔ domain mapping ONLY here (ADR-S007). Concrete over the filesystem
//! adapter — the wiring boundary is `main`, and nothing else varies.

mod dto;
pub mod routes;

pub use routes::{LiveCatalogService, routes};
