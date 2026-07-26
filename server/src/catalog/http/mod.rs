//! The catalog's inbound HTTP adapter: axum routes → the service, with wire DTO ↔ domain
//! mapping done ONLY here. Concrete over the filesystem adapter — the wiring boundary is
//! `main`, and nothing else varies.

pub mod admin;
pub mod c4;
mod dto;
pub mod routes;

pub use routes::{LiveCatalogService, routes};
