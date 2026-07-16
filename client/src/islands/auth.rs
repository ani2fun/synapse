//! The `@auth` island (oracle: `KeycloakJs.scala` over keycloak-js). One `boot` runs the
//! check-sso PKCE handshake; the handle lives in the app-level `AuthStore` for the session.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "@auth/loader")]
extern "C" {
    #[wasm_bindgen(js_name = bootAuth)]
    fn boot_auth_js(url: &str, realm: &str, client_id: &str) -> js_sys::Promise;

    pub type AuthHandle;
    #[wasm_bindgen(method, getter, js_name = authenticated)]
    fn authenticated_js(this: &AuthHandle) -> bool;
    #[wasm_bindgen(method, js_name = token)]
    fn token_js(this: &AuthHandle) -> Option<String>;
    #[wasm_bindgen(method, js_name = login)]
    fn login_js(this: &AuthHandle);
    #[wasm_bindgen(method, js_name = logout)]
    fn logout_js(this: &AuthHandle, redirect_uri: &str);
    #[wasm_bindgen(method, js_name = updateToken)]
    fn update_token_js(this: &AuthHandle, min_validity: u32) -> js_sys::Promise;
    #[wasm_bindgen(method, js_name = accountUrl)]
    fn account_url_js(this: &AuthHandle) -> String;
}

impl AuthHandle {
    pub fn authenticated(&self) -> bool {
        self.authenticated_js()
    }
    pub fn token(&self) -> Option<String> {
        self.token_js()
    }
    pub fn login(&self) {
        self.login_js();
    }
    pub fn logout(&self, redirect_uri: &str) {
        self.logout_js(redirect_uri);
    }
    /// Refresh when fewer than `min_validity` seconds remain; `Err` = the session is gone.
    pub async fn update_token(&self, min_validity: u32) -> Result<(), JsValue> {
        wasm_bindgen_futures::JsFuture::from(self.update_token_js(min_validity)).await?;
        Ok(())
    }
    pub fn account_url(&self) -> String {
        self.account_url_js()
    }
}

/// Run the check-sso PKCE handshake against the realm.
pub async fn boot(url: &str, realm: &str, client_id: &str) -> Result<AuthHandle, JsValue> {
    let handle = wasm_bindgen_futures::JsFuture::from(boot_auth_js(url, realm, client_id)).await?;
    Ok(handle.unchecked_into())
}
