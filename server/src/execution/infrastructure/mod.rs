//! The execution adapters — the go-judge wire protocol, per-language recipes, the Java
//! entrypoint normaliser, and the HTTP runner.

pub(crate) mod java_rewriter;
pub(crate) mod recipe;
mod runner;
pub(crate) mod wire;

pub use runner::GoJudgeRunner;
/// Re-exported so the router's edge timeout can be checked against it — the edge must outlast
/// the sandbox, or a slow-but-valid run dies at the door (`platform::limits`). Test-only by
/// construction: the ONE consumer is that invariant's test, so the alias is `cfg(test)`.
#[cfg(test)]
pub(crate) use runner::REQUEST_TIMEOUT as GO_JUDGE_REQUEST_TIMEOUT;
