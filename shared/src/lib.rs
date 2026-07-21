//! The shared kernel: the wire contract — the DTOs and pure models BOTH sides need.
//! Compiles native (server + fast tests) and to `wasm32`. Contexts grow their own modules
//! here as their DTOs land.
//!
//! The viz engine lives in the separate `viz-wasm` crate, not here: the server never
//! references it, and it is pure/DOM-free logic that an `rlib` already compiles and tests
//! natively without needing a home in the wire-contract kernel.
//!
//! What remains is a genuine kernel: a small crate whose only dependency is `serde`.

pub mod api;
pub mod blog;
pub mod catalog;
pub mod execution;
pub mod identity;
pub mod insights;
pub mod submission;
pub mod tutor;
