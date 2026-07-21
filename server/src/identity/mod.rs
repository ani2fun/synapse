//! The identity bounded context — bearer verification against the
//! Keycloak realm's JWKS. Other contexts consume the APPLICATION service and re-state their own
//! 401/503 mapping — never this context's http adapter. Account deletion
//! (`KeycloakAdmin`) is a separate capability layered on top.

pub mod application;
pub mod domain;
pub mod http;
pub mod infrastructure;
