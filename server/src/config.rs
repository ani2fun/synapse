//! Typed server config (oracle: `AppConfig.scala`). Defaults in code, overridden by `SYNAPSE_*`
//! env vars — deliberately NOT the bare `PORT`, which preview tooling injects and must never
//! hijack the server (the launch.json `unset PORT` gotcha, qna). Fields join one slice at a time,
//! exactly as the oracle grew them (ADR-S019).

use figment::Figment;
use figment::providers::{Env, Serialized};
use serde::{Deserialize, Serialize};

/// The whole server configuration. Step 01 carries only `port`; the catalog's `content_root`,
/// the executor URL, the database, identity, rate limits, … arrive with their slices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// TCP port the server binds (dev convention: 8180, same as the oracle). Env: `SYNAPSE_PORT`.
    pub port: u16,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { port: 8180 }
    }
}

impl AppConfig {
    /// Defaults merged with `SYNAPSE_`-prefixed env overrides (`SYNAPSE_PORT=9999`).
    /// (Boxed error: `figment::Error` is ~200 bytes and this sits on every caller's happy path.)
    pub fn load() -> Result<Self, Box<figment::Error>> {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Env::prefixed("SYNAPSE_"))
            .extract()
            .map_err(Box::new)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTS
// ─────────────────────────────────────────────────────────────────────────────
// `result_large_err`: the Jail closure's signature is figment's, not ours.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::result_large_err)]
mod tests {
    use super::*;

    #[test]
    fn defaults_bind_the_dev_port() {
        assert_eq!(AppConfig::default().port, 8180);
    }

    #[test]
    fn env_overrides_use_the_synapse_prefix() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("SYNAPSE_PORT", "9999");
            // The bare PORT the preview harness injects must be ignored.
            jail.set_env("PORT", "1234");
            let cfg = AppConfig::load().map_err(|e| *e)?;
            assert_eq!(cfg.port, 9999);
            Ok(())
        });
    }
}
