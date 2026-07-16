//! The submission bounded context (oracle: `server/submission/`) — the aggregate + the async
//! judging pipeline. Consumes the execution context's OWN `RunCodeService` (customer–supplier,
//! never a duplicated runner).

pub mod application;
pub mod domain;
