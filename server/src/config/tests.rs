//! Configuration is read once at startup and never again, so every default and every `SYNAPSE_*`
//! override is pinned here. The empty-string cases carry the most weight: figment reads an
//! exported-but-blank variable as `Some("")`, which is not the same as absent.

// `result_large_err`: the Jail closure's signature is figment's, not ours.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::result_large_err)]

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
fn platform_and_rate_limit_envs_are_read_without_the_synapse_prefix() {
    figment::Jail::expect_with(|jail| {
        jail.set_env("RATE_LIMIT_ANON_LIMIT", "3");
        let cfg = AppConfig::load().map_err(|e| *e)?;
        assert_eq!(cfg.rate_limit_anon_limit, 3);
        assert_eq!(cfg.rate_limit_anon_window_seconds, 60, "default stays");
        Ok(())
    });
}

#[test]
fn an_empty_astro_url_reads_as_no_page_tier() {
    figment::Jail::expect_with(|jail| {
        jail.set_env("SYNAPSE_ASTRO_URL", "");
        let cfg = AppConfig::load().map_err(|e| *e)?;
        assert_eq!(
            cfg.astro_url,
            Some(String::new()),
            "figment reads an empty env var as Some(\"\") — the premise this guards"
        );
        assert_eq!(
            cfg.astro_url(),
            None,
            "an empty origin must not mount a proxy pointed at nowhere"
        );
        Ok(())
    });
}

#[test]
fn a_real_astro_url_survives_the_blank_check() {
    let cfg = AppConfig {
        astro_url: Some("  http://127.0.0.1:4321  ".to_owned()),
        ..AppConfig::default()
    };
    assert_eq!(cfg.astro_url(), Some("http://127.0.0.1:4321"));
}

#[test]
fn admin_users_canonicalise_to_a_lowercase_set() {
    let cfg = AppConfig {
        admin_users: " Ada, GRACE ,, tester ".to_owned(),
        ..AppConfig::default()
    };
    let set = cfg.admin_user_set();
    assert_eq!(set.len(), 3);
    // Looked up the way the gate looks up — through the type, so a caller arriving as
    // "ADA" finds the entry configured as " Ada ".
    for raw in ["ADA", "grace", " TeStEr "] {
        let name = Username::parse(raw).expect("a non-blank name");
        assert!(set.contains(&name), "{raw} should be an admin");
    }
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
fn content_editing_defaults_to_a_credential_free_dry_run() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.content_forge, "dry-run");
    assert_eq!(cfg.content_repo, "ani2fun/synapse-content");
    assert!(cfg.github_token.is_empty(), "no token is ever a default");
    figment::Jail::expect_with(|jail| {
        jail.set_env("CONTENT_FORGE", "github");
        jail.set_env("GITHUB_TOKEN", "ghp_example");
        let cfg = AppConfig::load().map_err(|e| *e)?;
        assert_eq!(cfg.content_forge, "github");
        assert_eq!(cfg.github_token, "ghp_example");
        assert_eq!(cfg.content_repo_branch, "main", "default stays");
        Ok(())
    });
}

#[test]
fn synapse_root_maps_onto_content_root() {
    // A naive serde alias collides with the serialized default
    // ("duplicate field") — this pins the figment key mapping.
    figment::Jail::expect_with(|jail| {
        jail.set_env("SYNAPSE_ROOT", "/srv/content");
        let cfg = AppConfig::load().map_err(|e| *e)?;
        assert_eq!(cfg.content_root, "/srv/content");
        assert!(cfg.auto_reload, "default stays");
        Ok(())
    });
}
