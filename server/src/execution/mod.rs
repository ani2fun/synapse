//! The `execution` bounded context (oracle steps 09–11): run untrusted lesson code in the
//! go-judge sandbox. `domain/` is the pure language model; `application/` validates and owns
//! the `CodeRunner` port; the go-judge adapter and `POST /api/run` arrive in step 10.

pub mod application;
pub mod domain;
