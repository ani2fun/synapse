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
    /// TCP port the server binds (dev convention: 8280 — synapse-rs owns its own port pair,
    /// 5373/8280, so the Scala oracle's dev loop on 5273/8180 runs beside it). Env:
    /// `SYNAPSE_PORT`.
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
    /// The production SPA dist dir (step 18). Absent (the dev default) → no static routes;
    /// Vite serves the client. Env: `STATIC_ROOT`.
    pub static_root: String,
    /// The Astro SSR sidecar's origin (migration step A01). `Some` switches page serving from
    /// the built-in `StaticRoutes` to the `astro_proxy` fallback; `None` (the default) keeps
    /// today's behaviour exactly. Env: `SYNAPSE_ASTRO_URL` (or the bare `ASTRO_URL`).
    pub astro_url: Option<String>,
    /// The site's public origin, used for `<link rel="canonical">` and the Open Graph URLs the
    /// server injects (step 50). OG requires ABSOLUTE urls, and the `Host` header is
    /// caller-controlled — a configured value cannot be poisoned by a request.
    /// Env: `SYNAPSE_SITE_URL` (or the bare `SITE_URL`).
    pub site_url: String,
    /// The LikeC4 upstream the `/c4` proxy forwards to (step 18). Prod gotcha: the image
    /// serves UNDER `/c4`, so the value ends in `/c4` and the stripped prefix cancels.
    /// Env: `LIKEC4_URL`.
    pub likec4_url: String,
    /// Anonymous run/submit budget: per-IP fixed window (step 18). Envs:
    /// `RATE_LIMIT_ANON_WINDOW_SECONDS` / `RATE_LIMIT_ANON_LIMIT`.
    pub rate_limit_anon_window_seconds: u64,
    pub rate_limit_anon_limit: u32,
    /// Signed-in budget: per-subject, deliberately bigger. Envs:
    /// `RATE_LIMIT_AUTH_WINDOW_SECONDS` / `RATE_LIMIT_AUTH_LIMIT`.
    pub rate_limit_auth_window_seconds: u64,
    pub rate_limit_auth_limit: u32,
    /// The submit gate (step 20): dev/personal instances stay open; prod flips it on. Env:
    /// `SUBMISSION_ALLOWLIST_ENFORCED`.
    pub submission_allowlist_enforced: bool,
    /// The SCOPED Keycloak service-account client for account deletion (step 20 — the audit
    /// HIGH: never the master-realm admin). Envs: `KEYCLOAK_ADMIN_CLIENT_ID` /
    /// `KEYCLOAK_ADMIN_CLIENT_SECRET` (dev realm file seeds `synapse-admin`/`dev-admin-secret`).
    pub keycloak_admin_client_id: String,
    pub keycloak_admin_client_secret: String,
    /// Who may manage the allowlist (step 21) — comma-separated usernames, compared lowercase.
    /// A raw string (not a list) so the env override stays a plain value. Env: `ADMIN_USERS`.
    pub admin_users: String,
    /// The local Socratic coach (step 22, ADR-S025) — OFF by default; when off, the chat
    /// route is never mounted. Envs: `TUTOR_ENABLED` / `TUTOR_URL` / `TUTOR_MODEL`.
    pub tutor_enabled: bool,
    pub tutor_url: String,
    pub tutor_model: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: 8280,
            content_root: "../synapse-content".to_owned(),
            auto_reload: true,
            executor_url: "http://localhost:5150".to_owned(),
            database_url: "postgres://synapse:synapse@localhost:5532/synapse_rs".to_owned(),
            identity_issuer: "http://localhost:8181/realms/synapse".to_owned(),
            identity_audience: "synapse-web".to_owned(),
            static_root: "client/dist".to_owned(),
            astro_url: None,
            site_url: "https://synapse.kakde.eu".to_owned(),
            likec4_url: "http://localhost:8190".to_owned(),
            rate_limit_anon_window_seconds: 60,
            rate_limit_anon_limit: 10,
            rate_limit_auth_window_seconds: 3600,
            rate_limit_auth_limit: 100,
            submission_allowlist_enforced: false,
            keycloak_admin_client_id: "synapse-admin".to_owned(),
            keycloak_admin_client_secret: "dev-admin-secret".to_owned(),
            admin_users: "tester".to_owned(),
            tutor_enabled: false,
            tutor_url: "http://localhost:11434".to_owned(),
            tutor_model: "llama3.1".to_owned(),
        }
    }
}

impl AppConfig {
    /// `ADMIN_USERS` as the canonical set: split on `,`, trim, lowercase, drop empties.
    pub fn admin_user_set(&self) -> std::collections::HashSet<String> {
        self.admin_users
            .split(',')
            .map(|u| u.trim().to_lowercase())
            .filter(|u| !u.is_empty())
            .collect()
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
        // The step-18 platform names (the oracle's deploy-manifest spellings, no prefix).
        let platform = Env::raw()
            .only(&["STATIC_ROOT", "LIKEC4_URL", "SITE_URL", "ASTRO_URL"])
            .map(|key| key.as_str().to_lowercase().into());
        let rate = Env::raw()
            .only(&[
                "RATE_LIMIT_ANON_WINDOW_SECONDS",
                "RATE_LIMIT_ANON_LIMIT",
                "RATE_LIMIT_AUTH_WINDOW_SECONDS",
                "RATE_LIMIT_AUTH_LIMIT",
            ])
            .map(|key| key.as_str().to_lowercase().into());
        let account = Env::raw()
            .only(&[
                "SUBMISSION_ALLOWLIST_ENFORCED",
                "KEYCLOAK_ADMIN_CLIENT_ID",
                "KEYCLOAK_ADMIN_CLIENT_SECRET",
                "ADMIN_USERS",
                "TUTOR_ENABLED",
                "TUTOR_URL",
                "TUTOR_MODEL",
            ])
            .map(|key| key.as_str().to_lowercase().into());
        Figment::from(Serialized::defaults(Self::default()))
            .merge(env)
            .merge(executor)
            .merge(database)
            .merge(oidc)
            .merge(platform)
            .merge(rate)
            .merge(account)
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
        assert_eq!(AppConfig::default().port, 8280);
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
    fn platform_and_rate_limit_envs_use_the_oracle_names() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("LIKEC4_URL", "http://synapse-likec4/c4");
            jail.set_env("RATE_LIMIT_ANON_LIMIT", "3");
            let cfg = AppConfig::load().map_err(|e| *e)?;
            assert_eq!(cfg.likec4_url, "http://synapse-likec4/c4");
            assert_eq!(cfg.rate_limit_anon_limit, 3);
            assert_eq!(cfg.rate_limit_anon_window_seconds, 60, "default stays");
            Ok(())
        });
    }

    #[test]
    fn admin_users_canonicalise_to_a_lowercase_set() {
        let cfg = AppConfig {
            admin_users: " Ada, GRACE ,, tester ".to_owned(),
            ..AppConfig::default()
        };
        let set = cfg.admin_user_set();
        assert_eq!(set.len(), 3);
        assert!(set.contains("ada") && set.contains("grace") && set.contains("tester"));
    }

    #[test]
    fn account_admin_defaults_pin_the_scoped_client() {
        // The dev realm file seeds exactly these; prod overrides via the sealed secret.
        let cfg = AppConfig::default();
        assert!(!cfg.submission_allowlist_enforced, "dev stays open");
        assert_eq!(cfg.keycloak_admin_client_id, "synapse-admin");
        assert_eq!(cfg.keycloak_admin_client_secret, "dev-admin-secret");
        figment::Jail::expect_with(|jail| {
            jail.set_env("SUBMISSION_ALLOWLIST_ENFORCED", "true");
            let cfg = AppConfig::load().map_err(|e| *e)?;
            assert!(cfg.submission_allowlist_enforced);
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
