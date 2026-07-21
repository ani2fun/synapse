//! The viz engine (RS-P7; oracle: `shared/viz`, ADR-S026/S027) — pure, IO-free, DOM-free.
//! This step lands the SPINE: the render contract (`graph`), the one authored vocabulary
//! (`vocabulary`), the structure→renderer dispatch (`render_family`), the role-colour palette
//! (`markers`), and the one playback state machine (`playback`). The adapt pipeline and the
//! geometry families join in their own steps, exactly as the oracle staged them.

pub mod adapt;
pub mod decoder;
pub mod geometry;
pub mod graph;
pub mod markers;
pub mod playback;
pub mod render_family;
pub mod shapes;
pub mod trace;
pub mod vocabulary;
