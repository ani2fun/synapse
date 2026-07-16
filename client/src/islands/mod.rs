//! The TS-island boundary — the ONLY place the client touches JavaScript modules (the narrow
//! interop rule: the oracle kept `@JSImport` to 11 files; we keep `wasm_bindgen(module = …)`
//! externs to this module tree). One sub-module per island alias.

pub mod auth;
pub mod editor;
pub mod markdown;
