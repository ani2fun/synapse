//! The shared kernel (RS001, mirroring Synapse's `shared/` crossproject): wire contracts + the
//! pure models both sides need. Compiles native (server + fast tests) and to `wasm32` (client,
//! from step 02). Contexts grow their own modules here as their DTOs land; the viz engine joins
//! in RS-P7.

pub mod api;
pub mod catalog;
pub mod execution;
