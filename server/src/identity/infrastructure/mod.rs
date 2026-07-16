//! The identity adapters — the JWKS token verifier (Keycloak account deletion joins with its
//! own step).

mod jwks;

pub use jwks::JwksTokenVerifier;
