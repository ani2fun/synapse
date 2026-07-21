//! The Postgres `ReadinessProbe` adapter.
//!
//! `platform` is a thin context and stays flat — layering is proportional to complexity —
//! so this adapter sits beside its port rather than under an `infrastructure/` of its own —
//! the same call the context already makes for the rate limiter and the proxy.
//!
//! Postgres is the ONLY hard dependency: the server fail-fasts without it at boot and cannot
//! serve submissions without it. go-judge, Keycloak and Ollama all degrade to honest error
//! responses, so they must NOT gate readiness — a judge outage should not pull the whole site
//! out of the load balancer.

use std::future::Future;
use std::pin::Pin;

use sqlx::PgPool;

use crate::platform::health::ReadinessProbe;

pub struct PgReadiness {
    pool: PgPool,
}

impl PgReadiness {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ReadinessProbe for PgReadiness {
    fn ping(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            sqlx::query("select 1")
                .execute(&self.pool)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
    }
}
