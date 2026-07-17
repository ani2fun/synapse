//! The runnable-block views (oracle: `Workbench` + `WorkbenchOutput`, step-11 scope).

mod codebench;
mod hydrate;
mod icons;
pub(crate) mod lazy;
mod practice;
mod runnable;
mod workbench;

pub use codebench::{CodebenchModal, CodebenchStore, hydrate_codebench_pills};
pub use hydrate::hydrate_workbenches;
pub use practice::hydrate_practices;
pub(crate) use practice::mount_solutions;
pub use runnable::RunnableBlock;
