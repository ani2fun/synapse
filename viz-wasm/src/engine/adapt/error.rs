//! Why an adaptation produced no drawable cases — a TYPED error, not a bare string: the
//! Visualise modal matches on the case to decide its UI and renders `message()` as the human
//! line.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VizError {
    #[error("The trace produced no steps.")]
    EmptyTrace,
    #[error("The trace stepped only through builder frames.")]
    OnlyBuilderFrames,
    #[error("Couldn't find a structure to visualise — name the variable with `viz=<structure>:<var>`.")]
    NoRoot,
    #[error("The chosen root never held a structure during the trace.")]
    RootNeverHeldStructure,
    #[error("The trace produced no call frames to visualise.")]
    NoCallFrames,
    #[error("Two nodes shared the id '{0}' in one step.")]
    DuplicateNodeId(String),
}

impl VizError {
    #[must_use]
    pub fn message(&self) -> String {
        self.to_string()
    }
}
