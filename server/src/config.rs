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
    /// The go-judge sandbox `POST /run` base URL (step 10). Env: `EXECUTOR_URL` (the oracle's
    /// deploy-manifest name, mapped in `load`) or `SYNAPSE_EXECUTOR_URL`.
    pub executor_url: String,
    /// The submissions store (step 14). Env: `DATABASE_URL` (the ecosystem convention, honored
    /// verbatim) or `SYNAPSE_DATABASE_URL`. The server FAILS FAST when Postgres is down
    /// (oracle parity — Keycloak degrades, Postgres does not).
    pub database_url: String,
    /// The OIDC issuer whose tokens we accept (step 16) — the Keycloak realm URL. Env:
    /// `OIDC_ISSUER` (oracle name) or `SYNAPSE_IDENTITY_ISSUER`. Keycloak-down DEGRADES (503
    /// on token paths) — it never blocks boot.
    pub identity_issuer: String,
    /// The client id expected in `aud`/`azp` (step 16). Env: `OIDC_AUDIENCE`.
    pub identity_audience: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: 8180,
            content_root: "../synapse-content".to_owned(),
            auto_reload: true,
            executor_url: "http://localhost:5150".to_owned(),
            database_url: "postgres://synapse:synapse@localhost:5532/synapse_rs".to_owned(),
            identity_issuer: "http://localhost:8181/realms/synapse".to_owned(),
            identity_audience: "synapse-web".to_owned(),
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
        // `EXECUTOR_URL` is the oracle's deploy-manifest name (no prefix) — honored verbatim.
        let executor = Env::raw().only(&["EXECUTOR_URL"]).map(|_| "executor_url".into());
        let database = Env::raw().only(&["DATABASE_URL"]).map(|_| "database_url".into());
        let oidc = Env::raw().only(&["OIDC_ISSUER", "OIDC_AUDIENCE"]).map(|key| {
            if key == "OIDC_ISSUER" {
                "identity_issuer".into()
            } else {
                "identity_audience".into()
            }
        });
        Figment::from(Serialized::defaults(Self::default()))
            .merge(env)
            .merge(executor)
            .merge(database)
            .merge(oidc)
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
