//! The identity use case + ports: `IdentityService` + `TokenVerifier` + `KeycloakAdmin`.

use crate::identity::domain::AuthenticatedUser;

/// The two-way split every consumer leans on: a BAD token is the caller's problem (401); an
/// UNREACHABLE verifier is OURS (503) — IdP-down must never read as "invalid credentials".
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("invalid bearer token: {0}")]
    InvalidToken(String),
    #[error("token verifier unavailable: {0}")]
    VerifierUnavailable(String),
}

/// The outbound port the JWKS adapter implements.
pub trait TokenVerifier: Send + Sync {
    fn verify(&self, token: &str) -> impl Future<Output = Result<AuthenticatedUser, AuthError>> + Send;
}

/// The outbound port for account administration: ONE capability —
/// delete a user by `sub`. A missing user counts as already gone; any Keycloak-admin failure
/// is `VerifierUnavailable` (503 — the IdP being down is OUR problem, never a silent success).
pub trait KeycloakAdmin: Send + Sync {
    fn delete_user(&self, sub: &str) -> impl Future<Output = Result<(), AuthError>> + Send;
}

/// The driving service other contexts consume.
pub struct IdentityService<V, A> {
    verifier: V,
    admin: A,
}

impl<V: TokenVerifier, A: KeycloakAdmin> IdentityService<V, A> {
    pub fn new(verifier: V, admin: A) -> Self {
        Self { verifier, admin }
    }

    /// `skip_all` is not tidiness here, it is the whole point: `token` is a live bearer
    /// credential and the default `#[instrument]` behaviour is to record every argument.
    /// The username is recorded only AFTER verification succeeds, so an attacker's claimed
    /// identity never reaches the logs.
    #[tracing::instrument(name = "identity.authenticate", skip_all, fields(username))]
    pub async fn authenticate(&self, token: &str) -> Result<AuthenticatedUser, AuthError> {
        let user = self.verifier.verify(token).await?;
        tracing::Span::current().record("username", user.username.as_str());
        tracing::debug!("bearer verified");
        Ok(user)
    }

    /// Remove the caller's sign-in. App data is a SEPARATE verb (`DELETE /api/submissions`);
    /// the client orchestrates erase → delete so identity never depends on submissions.
    #[tracing::instrument(name = "identity.delete_account", skip(self))]
    pub async fn delete_account(&self, sub: &str) -> Result<(), AuthError> {
        tracing::info!("account: deleting Keycloak account");
        self.admin.delete_user(sub).await
    }
}
