//! Baseline security headers. Five headers on EVERY response — API, proxy, static, errors
//! alike; defence in depth is not just the happy path. The CSP is parameterised by the OIDC
//! issuer: it allows ONLY self + the Keycloak origin (+ the named third parties below), so a
//! misconfigured issuer surfaces as a broken sign-in, never as a wildcard policy.
//!
//! Two allowances this policy encodes, each earned by a real breakage (a CSP must be validated
//! against the app's HEAVIEST page, and only under prod-shaped serving — the dev server (Vite)
//! never sends these headers, so dev never reproduces a CSP-caused breakage):
//! - fonts/Monaco/inline-theme-script broke on an over-tight CSP → Google Fonts,
//!   `'unsafe-inline'`, `blob:` workers, `img-src https:` allowances;
//! - d2's blob render worker calls `new Function(elkJs)` at init (even under dagre), a blob
//!   worker INHERITS the page CSP, and no directive scopes eval to one worker →
//!   `'unsafe-eval'`. `'wasm-unsafe-eval'` covers WASM compilation only — here it is
//!   load-bearing for the viz-wasm engine (a Leptos-compiled WASM module that powers the
//!   visualisations), which the browser loads as a lazy WASM bundle.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;

const GOOGLE_FONTS_CSS: &str = "https://fonts.googleapis.com";
const GOOGLE_FONTS_FILE: &str = "https://fonts.gstatic.com";
const CF_INSIGHTS: &str = "https://static.cloudflareinsights.com";
const CF_BEACON_API: &str = "https://cloudflareinsights.com";

/// The precomputed header set — built once from the issuer at wiring time.
#[derive(Clone)]
pub struct SecurityHeaders {
    headers: Arc<Vec<(HeaderName, HeaderValue)>>,
}

impl SecurityHeaders {
    pub fn new(issuer: &str) -> Self {
        let csp = csp_for(&origin_of(issuer));
        let headers = vec![
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
            (header::X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN")),
            (
                header::REFERRER_POLICY,
                HeaderValue::from_static("strict-origin-when-cross-origin"),
            ),
            (
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_str(&csp)
                    .unwrap_or_else(|_| HeaderValue::from_static("default-src 'self'")),
            ),
            // Unconditional: Cloudflare terminates TLS, but stating HSTS at the origin keeps
            // the guarantee if the edge is ever bypassed.
            (
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_static("max-age=31536000; includeSubDomains"),
            ),
        ];
        Self {
            headers: Arc::new(headers),
        }
    }
}

/// The outermost stamp — appends the fixed set to whatever the inner stack produced.
pub async fn stamp(State(set): State<SecurityHeaders>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    for (name, value) in set.headers.iter() {
        response.headers_mut().insert(name.clone(), value.clone());
    }
    response
}

/// `scheme://host[:port]` of the issuer URL; empty (fail-open, logged) when unparseable —
/// the header still emits and sign-in breaking loudly beats no policy at all.
fn origin_of(issuer: &str) -> String {
    let Some((scheme, rest)) = issuer.split_once("://") else {
        tracing::warn!(
            issuer,
            "security headers: issuer has no scheme — CSP omits the auth origin"
        );
        return String::new();
    };
    let host = rest.split('/').next().unwrap_or_default();
    if scheme.is_empty() || host.is_empty() {
        tracing::warn!(
            issuer,
            "security headers: issuer unparseable — CSP omits the auth origin"
        );
        return String::new();
    }
    format!("{scheme}://{host}")
}

/// The policy in practice: `'wasm-unsafe-eval'` carries the viz-wasm engine, `'unsafe-eval'`
/// carries d2's ELK blob worker, `blob:`+`worker-src` carry Monaco/d2/mermaid/tracer workers,
/// and only the auth origin + named third parties join `'self'`.
fn csp_for(auth_origin: &str) -> String {
    [
        "default-src 'self'".to_owned(),
        format!("script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' blob: {CF_INSIGHTS}"),
        format!("style-src 'self' 'unsafe-inline' {GOOGLE_FONTS_CSS}"),
        "img-src 'self' data: https:".to_owned(),
        format!("font-src 'self' data: {GOOGLE_FONTS_FILE}"),
        format!("connect-src 'self' {auth_origin} {CF_BEACON_API} {CF_INSIGHTS}"),
        "worker-src 'self' blob:".to_owned(),
        format!("frame-src 'self' {auth_origin}"),
        "frame-ancestors 'self'".to_owned(),
        "base-uri 'self'".to_owned(),
        "object-src 'none'".to_owned(),
    ]
    .join("; ")
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
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
}
