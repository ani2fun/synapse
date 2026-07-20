//! The widget spine (oracle: `WidgetHost` + `RendererRegistry` + the SVG render families,
//! ADR-S028): one host consumes `VizCases` — whether it came from an authored
//! ` ```viz widget= ` fence or (later) the live-trace adapter — dispatches through the pure
//! `RenderFamily` decision, and drives every animation with the one `Playback` stepper.
//! Layout is computed ONCE over the union of steps; the step signal only toggles drawing.

pub mod blocks;
/// The pure viz ENGINE — contract, vocabulary, geometry, adapt pipeline and goldens.
/// Moved out of `synapse-shared` in step 45: the server referenced it zero times while it
/// made up 86% of that crate, so "shared" described the folder rather than the fact.
/// `shapes`/`decoder` moved INSIDE it in step 59 — they were pure engine logic sitting at
/// this level, where the purity gate could not see them.
pub mod engine;
pub mod host;
pub mod modal;
pub mod registry;
pub mod render;
pub mod session;
pub mod transport;
