//! Typed server config (oracle: `AppConfig.scala`). Defaults in code, overridden by `SYNAPSE_*`
//! env vars — deliberately NOT the bare `PORT`, which preview tooling injects and must never
//! hijack the server (the launch.json `unset PORT` gotcha, qna). Fields join one slice at a time,
//! exactly as the oracle grew them (ADR-S019).

use figment::Figment;
use figment::providers::{Env, Serialized};
use serde::{Deserialize, Serialize};

/// The whole server configuration — fields join one slice at a time (the executor URL, the
/// database, identity, rate limits, … arrive with their slices).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// TCP port the server binds (dev convention: 8180, same as the oracle). Env: `SYNAPSE_PORT`.
    pub port: u16,
    /// The synapse-content checkout (step 06). Env: `SYNAPSE_ROOT` (the oracle's name — mapped
    /// in `load`) or `SYNAPSE_CONTENT_ROOT`.
    pub content_root: String,
    /// ADR-S010: dev re-checks the content watermark so live edits show; prod builds the index
    /// once per git SHA. Env: `SYNAPSE_AUTO_RELOAD`.
    pub auto_reload: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: 8180,
            content_root: "../synapse-content".to_owned(),
            auto_reload: true,
        }
    }
}

impl AppConfig {
    /// Defaults merged with `SYNAPSE_`-prefixed env overrides (`SYNAPSE_PORT=9999`).
    /// (Boxed error: `figment::Error` is ~200 bytes and this sits on every caller's happy path.)
    pub fn load() -> Result<Self, Box<figment::Error>> {
        // `SYNAPSE_ROOT` is the oracle's env name for the content checkout — map it onto the
        // `content_root` field here (a serde alias would collide with the serialized default).
        let env = Env::prefixed("SYNAPSE_").map(|key| {
            if key == "root" {
                "content_root".into()
            } else {
                key.as_str().to_owned().into()
            }
        });
        Figment::from(Serialized::defaults(Self::default()))
            .merge(env)
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

    #[test]
    fn synapse_root_maps_onto_content_root() {
        // The oracle's env name; a naive serde alias collides with the serialized default
        // ("duplicate field") — this pins the figment key mapping.
        figment::Jail::expect_with(|jail| {
            jail.set_env("SYNAPSE_ROOT", "/srv/content");
            let cfg = AppConfig::load().map_err(|e| *e)?;
            assert_eq!(cfg.content_root, "/srv/content");
            assert!(cfg.auto_reload, "default stays");
            Ok(())
        });
    }
}
