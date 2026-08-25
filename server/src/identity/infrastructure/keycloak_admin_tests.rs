//! Splitting a Keycloak issuer URL into the admin base and the realm — the two halves every
//! admin call is assembled from.

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn the_issuer_splits_into_base_and_realm() {
    let client = KeycloakAdminClient::new("http://localhost:8181/realms/synapse/", "synapse-admin", "s");
    assert_eq!(client.base, "http://localhost:8181");
    assert_eq!(client.realm, "synapse");

    let odd = KeycloakAdminClient::new("http://plain-oidc.example", "synapse-admin", "s");
    assert_eq!(odd.realm, "master", "malformed degrades loudly, not silently");
}
