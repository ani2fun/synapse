//! The tutoring bounded context — the local-only Socratic coach: a single-hop proxy to an
//! OpenAI-compatible chat endpoint. Domain-free (a chat turn's role+content IS the whole
//! model); off by default and STRUCTURALLY excluded when disabled — the chat route is never
//! mounted.

pub mod application;
pub mod http;
pub mod infrastructure;
