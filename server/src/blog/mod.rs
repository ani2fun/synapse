//! The blog bounded context — a flat, chronological cousin of the
//! catalog: markdown posts in `<contentRoot>/blog/`, newest first, no tree.

pub mod application;
pub mod domain;
pub mod http;
pub mod infrastructure;
