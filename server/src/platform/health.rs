//! The `platform` context's health use cases, split the way
//! orchestrators actually consume them: a shallow **liveness** answer and a dependency-checking
//! **readiness** answer. Conflating the two is the classic operational footgun — see `status`.
//!
//! Liveness stays a free function (nothing to invert). Readiness declares the port the module
//! header always anticipated: it needs a real backing-store ping, so the seam finally exists.

use std::future::Future;
use std::pin::Pin;

use synapse_shared::api::HealthStatus;

/// Liveness: the process is up and serving. DELIBERATELY shallow — it must not consult Postgres
/// or any other dependency. A liveness probe that fails on a dependency blip makes the
/// orchestrator restart a perfectly healthy process, turning someone else's outage into a crash
/// loop. Dependency health is `ReadinessProbe`'s job.
pub fn status() -> HealthStatus {
    tracing::debug!("health check → ok");
    HealthStatus {
        status: "ok".to_owned(),
    }
}

/// Readiness: should this instance receive traffic right now?
///
/// Unlike liveness this consults a real dependency, which means dynamic dispatch at the router
/// edge — and `async fn` in traits is not dyn-safe. The hand-boxed future is the deliberate
/// exception to RS001's "no boxed futures where native AFIT works": here it does not.
pub trait ReadinessProbe: Send + Sync {
    /// `Ok(())` = ready. `Err(detail)` carries an OPERATOR-facing reason for the log only — it
    /// is never sent to the caller, since connection errors name hosts and usernames.
    fn ping(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;
}
