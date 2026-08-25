//! Typed server config. Defaults in code, overridden by `SYNAPSE_*`
//! env vars — deliberately NOT the bare `PORT`, which preview tooling injects and must never
//! hijack the server (the launch.json `unset PORT` gotcha, qna). Fields join one slice at a time,
//! one per feature area, so config grows alongside the features that need it.

/// One satellite mounted from a local directory rather than fetched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalSource {
    pub id: String,
    pub root: String,
    /// `/`-joined category slug path; absent is the top level.
    #[serde(default)]
    pub grouping: String,
    #[serde(default)]
    pub order: Option<i32>,
}

use figment::Figment;
use figment::providers::{Env, Serialized};
use serde::{Deserialize, Serialize};

use crate::identity::domain::Username;

/// The whole server configuration — fields join one slice at a time (the executor URL, the
/// database, identity, rate limits, … arrive with their slices).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// TCP port the server binds (dev convention: 8280 — synapse-rs owns its own port pair,
    /// 5373/8280, kept separate from other local dev services). Env:
    /// `SYNAPSE_PORT`.
    pub port: u16,
    /// The spine checkout — the git-sync'd `synapse-content`, mounted first and always. Env:
    /// `SYNAPSE_ROOT` (mapped in `load`) or `SYNAPSE_CONTENT_ROOT`.
    ///
    /// The default is a DEV convenience and assumes the sibling layout every content repository
    /// now shares: `synapse-content-github/` holding the spine beside each satellite guide. Prod
    /// sets `SYNAPSE_ROOT` at git-sync's mount, and `dev-tools/e2e` points at its own fixture, so
    /// nothing but a local `dev-tools/dev` reads this value.
    pub content_root: String,
    /// Dev re-checks the content watermark so live edits show; prod builds the index
    /// once per git SHA. Env: `SYNAPSE_AUTO_RELOAD`.
    pub auto_reload: bool,
    /// The go-judge sandbox `POST /run` base URL. Env: `EXECUTOR_URL` (the
    /// deploy-manifest name, mapped in `load`) or `SYNAPSE_EXECUTOR_URL`.
    pub executor_url: String,
    /// The submissions store. Env: `DATABASE_URL` (the ecosystem convention, honored
    /// verbatim) or `SYNAPSE_DATABASE_URL`. The server FAILS FAST when Postgres is down
    /// (Keycloak degrades gracefully instead; Postgres, as the system of record, does not).
    pub database_url: String,
    /// The OIDC issuer whose tokens we accept — the Keycloak realm URL. Env:
    /// `OIDC_ISSUER` or `SYNAPSE_IDENTITY_ISSUER`. Keycloak-down DEGRADES (503
    /// on token paths) — it never blocks boot.
    pub identity_issuer: String,
    /// The client id expected in `aud`/`azp`. Env: `OIDC_AUDIENCE`.
    pub identity_audience: String,
    /// The Astro SSR sidecar's origin. `Some` mounts the page proxy as the router fallback;
    /// `None` (the dev default) serves the API alone. Env: `SYNAPSE_ASTRO_URL` (or the bare
    /// `ASTRO_URL`).
    pub astro_url: Option<String>,
    /// The site's public origin, used for the sitemap's absolute URLs.
    /// Env: `SYNAPSE_SITE_URL` (or the bare `SITE_URL`).
    pub site_url: String,
    /// The LikeC4 upstream the `/c4` proxy forwards to. Prod gotcha: the image
    /// serves UNDER `/c4`, so the value ends in `/c4` and the stripped prefix cancels.
    /// Env: `LIKEC4_URL`.
    pub likec4_url: String,
    /// Anonymous run/submit budget: per-IP fixed window. Envs:
    /// `RATE_LIMIT_ANON_WINDOW_SECONDS` / `RATE_LIMIT_ANON_LIMIT`.
    pub rate_limit_anon_window_seconds: u64,
    pub rate_limit_anon_limit: u32,
    /// Signed-in budget: per-subject, deliberately bigger. Envs:
    /// `RATE_LIMIT_AUTH_WINDOW_SECONDS` / `RATE_LIMIT_AUTH_LIMIT`.
    pub rate_limit_auth_window_seconds: u64,
    pub rate_limit_auth_limit: u32,
    /// The submit gate: dev/personal instances stay open; prod flips it on. Env:
    /// `SUBMISSION_ALLOWLIST_ENFORCED`.
    pub submission_allowlist_enforced: bool,
    /// The SCOPED Keycloak service-account client for account deletion (least privilege:
    /// never the master-realm admin). Envs: `KEYCLOAK_ADMIN_CLIENT_ID` /
    /// `KEYCLOAK_ADMIN_CLIENT_SECRET` (dev realm file seeds `synapse-admin`/`dev-admin-secret`).
    pub keycloak_admin_client_id: String,
    pub keycloak_admin_client_secret: String,
    /// Who may manage the allowlist — comma-separated usernames, compared lowercase.
    /// A raw string (not a list) so the env override stays a plain value. Env: `ADMIN_USERS`.
    pub admin_users: String,
    /// The local Socratic coach — OFF by default; when off, the chat
    /// route is never mounted. Envs: `TUTOR_ENABLED` / `TUTOR_URL` / `TUTOR_MODEL`.
    pub tutor_enabled: bool,
    pub tutor_url: String,
    pub tutor_model: String,
    /// In-app prose editing: `off` (the routes are never mounted — a structural 404, the coach's
    /// pattern) · `dry-run` (the whole flow runs, nothing leaves the process) · `github` (real
    /// pull requests). Env: `CONTENT_FORGE`.
    pub content_forge: String,
    /// The content repository proposals target, `owner/name`, and its default branch. Envs:
    /// `CONTENT_REPO` / `CONTENT_REPO_BRANCH`.
    pub content_repo: String,
    pub content_repo_branch: String,
    /// Where fetched satellite checkouts live. One directory per registered source, each holding
    /// its commits and a `current` symlink — git-sync's layout, because a reader must never see a
    /// half-written tree. An `emptyDir` in production: a cold boot simply refetches. Env:
    /// `SYNAPSE_CONTENT_CACHE`.
    pub content_cache: String,
    /// Satellites mounted straight off local disk, as JSON:
    /// `[{"id":"java","root":"../java-guide","grouping":"programming-languages","order":7}]`.
    ///
    /// DEV AND TEST ONLY, and it earns its place twice. An author working on a guide repository
    /// wants to see it in the running app without pushing and waiting for a fetch; and the e2e
    /// suite needs a satellite in the library without reaching the network, which no registry row
    /// can give it. A local path cannot live in a shared registry, so this is the one thing the
    /// database genuinely cannot express. Env: `SYNAPSE_LOCAL_SOURCES`.
    pub local_sources: String,
    /// Seconds between reconciles of the source registry against disk. Matches git-sync's cadence
    /// for the primary checkout, which is what the content `max-age` is tuned to. `0` disables the
    /// loop entirely — satellites then never sync, which is the right shape for tests.
    /// Env: `SYNAPSE_CONTENT_SYNC_SECONDS`.
    pub content_sync_seconds: u64,
    /// The fine-grained PAT the forge commits with — `contents: write` +
    /// `pull_requests: write` on `content_repo` ALONE. Never logged, never returned, never sent
    /// anywhere but api.github.com. Empty with `content_forge = "github"` degrades LOUDLY to a
    /// dry run rather than silently accepting edits it cannot forward. Env: `GITHUB_TOKEN`.
    pub github_token: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: 8280,
            content_root: "../synapse-content-github/synapse-content".to_owned(),
            auto_reload: true,
            executor_url: "http://localhost:5150".to_owned(),
            database_url: "postgres://synapse:synapse@localhost:5532/synapse_rs".to_owned(),
            identity_issuer: "http://localhost:8181/realms/synapse".to_owned(),
            identity_audience: "synapse-web".to_owned(),
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
            // Dev gets the whole editing flow WITHOUT credentials: the gate, the drift guard, the
            // validation, the branch derivation and the stored history all run for real, and only
            // the forge call at the end is skipped. `off` is for a deployment that wants the
            // routes gone entirely.
            content_forge: "dry-run".to_owned(),
            content_repo: "ani2fun/synapse-content".to_owned(),
            content_repo_branch: "main".to_owned(),
            content_cache: "../.synapse-content-cache".to_owned(),
            content_sync_seconds: 60,
            local_sources: String::new(),
            github_token: String::new(),
        }
    }
}

impl AppConfig {
    /// `ADMIN_USERS` as the canonical set: split on `,`, trim, lowercase, drop empties.
    /// The locally-mounted satellites. Blank is none; malformed is a BOOT FAILURE rather than a
    /// silent skip — half a library served as if it were whole is the worse outcome.
    pub fn local_sources(&self) -> Result<Vec<LocalSource>, serde_json::Error> {
        if self.local_sources.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&self.local_sources)
    }

    /// The page tier's origin, or `None` when this deployment serves the API alone.
    ///
    /// BLANK IS ABSENT, and that is the whole point of reading it through here. figment
    /// treats an unset variable as absent but an EMPTY one as `Some("")`, which mounts the
    /// page proxy pointed at nowhere: every page 502s while `/api/health` stays green, so it
    /// reads as a content outage rather than the config typo it is. `dev-tools/start.sh`
    /// unsets the variable before exec'ing for exactly this reason — this is the same
    /// guarantee for every other way the binary can be launched.
    pub fn astro_url(&self) -> Option<&str> {
        self.astro_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
    }

    /// `ADMIN_USERS` as the gate compares it. Canonicalisation is [`Username`]'s, not this
    /// function's — a set built to a second recipe is a set that stops matching the verifier.
    pub fn admin_user_set(&self) -> std::collections::HashSet<Username> {
        self.admin_users.split(',').filter_map(Username::parse).collect()
    }
}

impl AppConfig {
    /// Defaults merged with `SYNAPSE_`-prefixed env overrides (`SYNAPSE_PORT=9999`).
    /// (Boxed error: `figment::Error` is ~200 bytes and this sits on every caller's happy path.)
    pub fn load() -> Result<Self, Box<figment::Error>> {
        // `SYNAPSE_ROOT` is the env name for the content checkout — map it onto the
        // `content_root` field here (a serde alias would collide with the serialized default).
        let env = Env::prefixed("SYNAPSE_").map(|key| {
            if key == "root" {
                "content_root".into()
            } else {
                key.as_str().to_owned().into()
            }
        });
        // `EXECUTOR_URL` is the deploy-manifest name (no prefix) — honored verbatim.
        let executor = Env::raw().only(&["EXECUTOR_URL"]).map(|_| "executor_url".into());
        let database = Env::raw().only(&["DATABASE_URL"]).map(|_| "database_url".into());
        let oidc = Env::raw().only(&["OIDC_ISSUER", "OIDC_AUDIENCE"]).map(|key| {
            if key == "OIDC_ISSUER" {
                "identity_issuer".into()
            } else {
                "identity_audience".into()
            }
        });
        // Deploy-manifest spellings accepted without the SYNAPSE_ prefix.
        let platform = Env::raw()
            .only(&["LIKEC4_URL", "SITE_URL", "ASTRO_URL"])
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
                "CONTENT_FORGE",
                "CONTENT_REPO",
                "CONTENT_REPO_BRANCH",
                "GITHUB_TOKEN",
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

#[cfg(test)]
mod tests;
