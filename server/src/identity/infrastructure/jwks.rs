//! The JWKS token verifier (oracle: `JwksTokenVerifier.scala`). The pipeline, verbatim:
//! RS256 against the realm's JWKS (lazy first fetch, ~5 min cache, ONE forced refresh on an
//! unknown `kid`), exact `iss`, `exp` with 60 s leeway, required `{sub, exp}` — then the MANUAL
//! Keycloak audience quirk: public SPA tokens carry `aud:["account"]` and name the client in
//! `azp`, so the rule is `aud ∋ clientId OR azp == clientId` (nimbus/jsonwebtoken `aud`
//! checking stays OFF). Usernames leave here CANONICAL LOWERCASE (step-36 audit fix, applied
//! once). Degrade: JWKS unreachable → `VerifierUnavailable` (503); everything else →
//! `InvalidToken` (401).

use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::identity::application::{AuthError, TokenVerifier};
use crate::identity::domain::{AuthenticatedUser, UserId};

const CACHE_TTL: Duration = Duration::from_mins(5);
const CLOCK_SKEW_SECONDS: u64 = 60;

pub struct JwksTokenVerifier {
    jwks_url: String,
    issuer: String,
    audience: String,
    client: reqwest::Client,
    cache: RwLock<Option<(Instant, JwkSet)>>,
}

impl JwksTokenVerifier {
    /// `issuer` is the realm URL (`…/realms/synapse`); the JWKS lives at the OIDC certs path.
    pub fn new(issuer: &str, audience: &str) -> Self {
        let issuer = issuer.trim_end_matches('/').to_owned();
        Self {
            jwks_url: format!("{issuer}/protocol/openid-connect/certs"),
            issuer,
            audience: audience.to_owned(),
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            cache: RwLock::new(None),
        }
    }

    /// The cached key set, refetched past the TTL or when `force` (unknown kid) demands it.
    async fn key_set(&self, force: bool) -> Result<JwkSet, AuthError> {
        if !force
            && let Some((fetched_at, set)) = &*self.cache.read().await
            && fetched_at.elapsed() < CACHE_TTL
        {
            return Ok(set.clone());
        }
        let set: JwkSet = self
            .client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| AuthError::VerifierUnavailable(e.to_string()))?
            .error_for_status()
            .map_err(|e| AuthError::VerifierUnavailable(e.to_string()))?
            .json()
            .await
            .map_err(|e| AuthError::VerifierUnavailable(e.to_string()))?;
        *self.cache.write().await = Some((Instant::now(), set.clone()));
        Ok(set)
    }

    /// Find the token's key: cache first, then ONE forced refresh (key rotation shows up as an
    /// unknown kid; a still-unknown kid after refresh is the caller's problem).
    async fn key_for(&self, kid: &str) -> Result<DecodingKey, AuthError> {
        for force in [false, true] {
            let set = self.key_set(force).await?;
            if let Some(jwk) = set.find(kid) {
                return DecodingKey::from_jwk(jwk)
                    .map_err(|e| AuthError::InvalidToken(format!("unusable JWKS key: {e}")));
            }
        }
        Err(AuthError::InvalidToken(format!("no JWKS key for kid '{kid}'")))
    }
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    /// Keycloak may emit a single string or an array.
    #[serde(default)]
    aud: Option<serde_json::Value>,
    #[serde(default)]
    azp: Option<String>,
    #[serde(default)]
    preferred_username: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

impl TokenVerifier for JwksTokenVerifier {
    async fn verify(&self, token: &str) -> Result<AuthenticatedUser, AuthError> {
        let header =
            jsonwebtoken::decode_header(token).map_err(|e| AuthError::InvalidToken(e.to_string()))?;
        let kid = header
            .kid
            .ok_or_else(|| AuthError::InvalidToken("token names no kid".to_owned()))?;
        let key = self.key_for(&kid).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_required_spec_claims(&["sub", "exp", "iss"]);
        validation.leeway = CLOCK_SKEW_SECONDS;
        validation.validate_aud = false; // the manual Keycloak quirk below

        let data = jsonwebtoken::decode::<Claims>(token, &key, &validation)
            .map_err(|e| AuthError::InvalidToken(e.to_string()))?;
        check_audience(&data.claims, &self.audience)?;
        Ok(to_user(data.claims))
    }
}

/// `aud ∋ clientId OR azp == clientId` — public SPA tokens carry `aud:["account"]`.
fn check_audience(claims: &Claims, audience: &str) -> Result<(), AuthError> {
    let aud_hit = match &claims.aud {
        Some(serde_json::Value::String(s)) => s == audience,
        Some(serde_json::Value::Array(items)) => items.iter().any(|v| v.as_str() == Some(audience)),
        _ => false,
    };
    let azp_hit = claims.azp.as_deref() == Some(audience);
    if aud_hit || azp_hit {
        Ok(())
    } else {
        Err(AuthError::InvalidToken(format!(
            "token is not for '{audience}' (aud/azp)"
        )))
    }
}

/// Username: `preferred_username` (non-empty) else `sub` — then LOWERCASE, here, once.
fn to_user(claims: Claims) -> AuthenticatedUser {
    let username = claims
        .preferred_username
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| claims.sub.clone())
        .to_lowercase();
    AuthenticatedUser {
        id: UserId(claims.sub),
        username,
        email: claims.email.filter(|e| !e.is_empty()),
    }
}
