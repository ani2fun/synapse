//! The Synapse client, Rust edition — Leptos (CSR), a UI on the three-layer rule (pure `logic/`
//! → reactive `state/` → `view/`) by feature, mirroring ADR-S014. Step 02 is the walking
//! skeleton: the router shell, the first TS-island round trip, and the shared-DTO proof — the
//! layers grow with the features.
//!
//! Interop (RS001): TS islands are bound in `islands/` via `wasm_bindgen(module = "@alias/…")`
//! externs — the Vite aliases resolve them, and each loader dynamic-imports its heavy renderer so
//! it stays in its own chunk, exactly like the oracle's `@JSImport` loader pattern.

pub mod api;
pub mod catalog;
pub mod execution;
pub mod islands;
pub mod router;
pub mod shell;

use wasm_bindgen::prelude::wasm_bindgen;

/// WASM entry — invoked once by the generated glue after `init()` in `main.ts`.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(shell::App);
}
