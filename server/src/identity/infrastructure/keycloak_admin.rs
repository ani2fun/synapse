//! The scoped Keycloak admin adapter (`KeycloakAdminClient`). It authenticates as a confidential
//! SERVICE-ACCOUNT client in OUR OWN realm (`synapse-admin`, `client_credentials`,
//! `realm-management:manage-users` only) — never the master-realm bootstrap admin, whose leak
//! would be a full IdP takeover. Two hops per delete: token, then
//! `DELETE /admin/realms/{realm}/users/{sub}` (204 = deleted, 404 = already gone — both success).

use crate::identity::application::{AuthError, KeycloakAdmin};

pub struct KeycloakAdminClient {
    client: reqwest::Client,
    /// The Keycloak base URL (issuer minus `/realms/{realm}`).
    base: String,
    realm: String,
    client_id: String,
    client_secret: String,
}

impl KeycloakAdminClient {
    /// `issuer` is the realm URL (`…/realms/synapse`); a malformed one degrades to
    /// `(issuer, "master")` and fails LOUDLY at call time.
    pub fn new(issuer: &str, client_id: &str, client_secret: &str) -> Self {
        let trimmed = issuer.trim_end_matches('/');
        let (base, realm) = match trimmed.split_once("/realms/") {
            Some((base, realm)) if !base.is_empty() && !realm.is_empty() => {
                (base.to_owned(), realm.to_owned())
            }
            _ => (trimmed.to_owned(), "master".to_owned()),
        };
        let client = reqwest::Client::builder()
            .http1_only()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base,
            realm,
            client_id: client_id.to_owned(),
            client_secret: client_secret.to_owned(),
        }
    }

    async fn admin_token(&self) -> Result<String, AuthError> {
        let url = format!(
            "{}/realms/{}/protocol/openid-connect/token",
            self.base, self.realm
        );
        let response = self
            .client
            .post(&url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
            ])
            .send()
            .await
            .map_err(|e| unavailable(&format!("token request failed: {e}")))?;
        if !response.status().is_success() {
            return Err(unavailable(&format!(
                "token endpoint answered {}",
                response.status()
            )));
        }
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| unavailable(&format!("token response undecodable: {e}")))?;
        body.get("access_token")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or_else(|| unavailable("token response carried no access_token"))
    }
}

fn unavailable(detail: &str) -> AuthError {
    AuthError::VerifierUnavailable(format!("Keycloak admin API: {detail}"))
}

impl KeycloakAdmin for KeycloakAdminClient {
    async fn delete_user(&self, sub: &str) -> Result<(), AuthError> {
        let token = self.admin_token().await?;
        let url = format!("{}/admin/realms/{}/users/{sub}", self.base, self.realm);
        let response = self
            .client
            .delete(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| unavailable(&format!("delete request failed: {e}")))?;
        match response.status().as_u16() {
            // 404 = already gone — deleting twice is not an error.
            204 | 404 => {
                tracing::info!(sub, "account: Keycloak user deleted");
                Ok(())
            }
            other => {
                tracing::warn!(sub, status = other, "account: Keycloak user delete failed");
                Err(unavailable(&format!("delete answered {other}")))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_issuer_splits_into_base_and_realm() {
        let client = KeycloakAdminClient::new("http://localhost:8181/realms/synapse/", "synapse-admin", "s");
        assert_eq!(client.base, "http://localhost:8181");
        assert_eq!(client.realm, "synapse");

        let odd = KeycloakAdminClient::new("http://plain-oidc.example", "synapse-admin", "s");
        assert_eq!(odd.realm, "master", "malformed degrades loudly, not silently");
    }
}
