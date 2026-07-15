//! The `platform` context's one use case — a liveness check (oracle: `Health.scala`).
//!
//! A free function, not a trait: there is no output dependency to invert (nothing backs the
//! check yet), and a single-impl trait would be ceremony without a seam (the Rust anti-pattern
//! list bans `dyn` where nothing varies). The port arrives with the first real backing-store
//! ping, exactly as it did in the oracle.

use synapse_shared::api::HealthStatus;

/// Walking-skeleton stub: reports ok. (No backing stores are wired yet — they join
/// `HealthStatus` later.)
pub fn status() -> HealthStatus {
    tracing::debug!("health check → ok (walking skeleton)");
    HealthStatus {
        status: "ok (walking skeleton)".to_owned(),
    }
}
