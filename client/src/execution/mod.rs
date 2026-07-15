//! The execution feature (oracle: `client/execution/` — the ADR-S014 three-layer split): the
//! pure `CodeExecutor` FSM (`logic/`), per-block reactive stores (`state/`), and the runnable
//! code block UI over the `@editor` island (`view/`).

pub mod logic;
pub mod state;
pub mod view;
