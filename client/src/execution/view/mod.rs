//! The runnable-block views (oracle: `Workbench` + `WorkbenchOutput`, step-11 scope).

mod codebench;
mod fence_group;
mod hydrate;
mod icons;
pub(crate) mod lazy;
mod output;
mod practice;
mod runnable;
mod workbench;

pub use codebench::{CodebenchModal, CodebenchStore};
pub use fence_group::hydrate_fence_groups;
pub use hydrate::hydrate_workbenches;
pub(crate) use practice::SolutionViewer;
pub use practice::hydrate_practices;
pub use runnable::RunnableBlock;
pub use workbench::{CaseRequest, no_case_yet, use_case_button};
