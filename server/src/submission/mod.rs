//! The submission bounded context — the aggregate + the async judging pipeline. Consumes the
//! execution context's OWN `RunCodeService` (customer–supplier, never a duplicated runner).

pub mod application;
pub mod domain;
pub mod http;
pub mod infrastructure;
