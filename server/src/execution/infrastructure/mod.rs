//! The execution adapters (oracle: `execution/infrastructure/`) — the go-judge wire protocol,
//! per-language recipes, the Java entrypoint normaliser, and the HTTP runner.

pub mod java_rewriter;
pub mod recipe;
mod runner;
pub mod wire;

pub use runner::GoJudgeRunner;
/// Re-exported so the router's edge timeout can be checked against it — the edge must outlast
/// the sandbox, or a slow-but-valid run dies at the door (`platform::limits`).
pub use runner::REQUEST_TIMEOUT as GO_JUDGE_REQUEST_TIMEOUT;
