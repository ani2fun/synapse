//! The identity bounded context (oracle: `server/identity/`) — bearer verification against the
//! Keycloak realm's JWKS. Other contexts consume the APPLICATION service and re-state their own
//! 401/503 mapping (qna Q27) — never this context's http adapter. Account deletion
//! (`KeycloakAdmin`) joins with its own step.

pub mod application;
pub mod domain;
pub mod http;
pub mod infrastructure;
