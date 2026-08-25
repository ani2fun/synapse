//! The response headers a browser trusts us on. The CSP is assembled from the auth issuer, so
//! the unparseable-issuer case is the one that matters: it fails OPEN with a complete policy
//! rather than emitting one with a hole in it.

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn origin_of_keeps_scheme_host_and_port() {
    assert_eq!(
        origin_of("https://keycloak.kakde.eu/realms/synapse"),
        "https://keycloak.kakde.eu"
    );
    assert_eq!(
        origin_of("http://localhost:8181/realms/synapse"),
        "http://localhost:8181"
    );
    assert_eq!(origin_of("not a url"), "");
}

#[test]
fn the_csp_names_the_auth_origin_and_the_app_allowances() {
    let csp = csp_for("https://keycloak.kakde.eu");
    assert!(csp.contains("connect-src 'self' https://keycloak.kakde.eu"));
    assert!(csp.contains("frame-src 'self' https://keycloak.kakde.eu"));
    assert!(csp.contains("'wasm-unsafe-eval'"), "the Leptos app itself");
    assert!(csp.contains("'unsafe-eval'"), "d2's ELK blob worker");
    assert!(csp.contains("worker-src 'self' blob:"));
    assert!(csp.contains("font-src 'self' data: https://fonts.gstatic.com"));
    assert!(csp.contains("object-src 'none'"));
}

#[test]
fn an_unparseable_issuer_fails_open_without_a_gap() {
    // Sign-in would break loudly; the policy itself stays intact and single-spaced.
    let csp = csp_for(&origin_of("garbage"));
    assert!(csp.contains("connect-src 'self' https://cloudflareinsights.com"));
    assert!(!csp.contains("  "), "no double spaces from the empty origin");
}
