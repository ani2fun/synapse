//! The `execution` bounded context: run untrusted lesson code in the
//! go-judge sandbox. `domain/` is the pure language model; `application/` validates and owns
//! the `CodeRunner` port; `infrastructure/` holds the go-judge adapter and `http/` exposes
//! `POST /api/run`.

pub mod application;
pub mod domain;
pub mod http;
pub mod infrastructure;
