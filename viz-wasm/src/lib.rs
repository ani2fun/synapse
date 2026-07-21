//! The standalone viz crate (migration step A10): the widget spine (oracle: `WidgetHost` +
//! `RendererRegistry` + the SVG render families, ADR-S028), the trace session, and the
//! Visualise modal — one host consumes `VizCases`, dispatches through the pure `RenderFamily`
//! decision, and drives every animation with the one `Playback` stepper. Layout is computed
//! ONCE over the union of steps; the step signal only toggles drawing.
//!
//! Two consumers, one crate: the old Leptos client depends on it as an rlib (its `viz` module,
//! repointed — nothing there changed), and the Astro app loads the cdylib as a lazy wasm
//! bundle through [`entry`]'s wasm-bindgen surface. The six couplings the client used to
//! provide (mount kit, editor/tracer externs, the `/api/run` fetch + bearer seam, theme probe,
//! logger) live in-crate now — the crate is self-contained by construction.

pub mod api;
pub mod blocks;
/// The pure viz ENGINE — contract, vocabulary, geometry, adapt pipeline and goldens.
/// Moved out of `synapse-shared` in step 45: the server referenced it zero times while it
/// made up 86% of that crate, so "shared" described the folder rather than the fact.
/// `shapes`/`decoder` moved INSIDE it in step 59 — they were pure engine logic sitting at
/// this level, where the purity gate could not see them.
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
