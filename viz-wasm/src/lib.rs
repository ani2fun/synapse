//! The standalone viz crate: the widget spine (a host that consumes `VizCases`, dispatches
//! through the pure `RenderFamily` decision, and drives every animation with the one
//! `Playback` stepper), the trace session, and the Visualise modal. Layout is computed ONCE
//! over the union of steps; the step signal only toggles drawing.
//!
//! Two consumers, one crate: the Leptos client depends on it as an rlib (its `viz` module),
//! and the Astro app loads the cdylib as a lazy wasm bundle through [`entry`]'s wasm-bindgen
//! surface. Mounting, the editor/tracer externs, the `/api/run` fetch + bearer seam, the theme
//! probe, and the logger all live in-crate, so the crate is self-contained by construction —
//! neither host needs to supply anything beyond calling into it.

pub mod api;
pub mod blocks;
/// The pure viz ENGINE — contract, vocabulary, geometry, adapt pipeline and goldens.
/// Lives in this crate, not `synapse-shared`, because the server never references it and it
/// needs no home in the wire-contract kernel. `shapes`/`decoder` live inside it too — they are
/// pure engine logic, and keeping them here is what lets the purity gate cover them.
pub mod engine;
pub mod entry;
pub mod ffi;
pub mod host;
pub mod log;
pub mod modal;
pub mod mount;
pub mod registry;
pub mod render;
pub mod session;
pub mod theme;
pub mod transport;
