//! The identity use case + port (oracle: `IdentityService` + `TokenVerifier`).

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

/// The driving service other contexts consume.
pub struct IdentityService<V> {
    verifier: V,
}

impl<V: TokenVerifier> IdentityService<V> {
    pub fn new(verifier: V) -> Self {
        Self { verifier }
    }

    pub async fn authenticate(&self, token: &str) -> Result<AuthenticatedUser, AuthError> {
        let user = self.verifier.verify(token).await?;
        tracing::debug!(username = user.username, "bearer verified");
        Ok(user)
    }
}
