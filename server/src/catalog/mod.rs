//! The `catalog` bounded context — the reference hexagon that other contexts follow. Reads the
//! content tree (`SYNAPSE_ROOT` conventions) into the browsable catalog and serves lesson
//! payloads. `domain/` is pure (std + serde only — the greppable rule); `application/` declares
//! the `ContentRepository` port; `infrastructure/` walks the filesystem; `http/` maps wire DTOs.

pub mod application;
pub mod domain;
pub mod http;
pub mod infrastructure;
