//! The viz engine — pure, IO-free, DOM-free. The spine: the render contract (`graph`), the
//! one authored vocabulary (`vocabulary`), the structure→renderer dispatch (`render_family`),
//! the role-colour palette (`markers`), and the one playback state machine (`playback`).
//! The adapt pipeline (`adapt`) and the geometry families (`geometry`) turn a raw trace into
//! that render contract.

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
