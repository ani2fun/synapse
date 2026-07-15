//! The `catalog` bounded context — the reference hexagon walk (oracle steps 03–06, ADR-S007/S011).
//! Reads the content tree (`SYNAPSE_ROOT` conventions, ADR-S010) into the browsable catalog and
//! serves lesson payloads. `domain/` is pure (std + serde only — the greppable rule);
//! `application/` declares the `ContentRepository` port; `infrastructure/` walks the filesystem;
//! `http/` maps wire DTOs.

pub mod application;
pub mod domain;
pub mod http;
pub mod infrastructure;
