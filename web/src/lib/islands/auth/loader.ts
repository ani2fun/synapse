// ──────────────────────────────────────────────────────────────────
// AUTH LOADER — keycloak-js behind a dynamic import
// ──────────────────────────────────────────────────────────────────
// A thin async wrapper around keycloak-js: one bootAuth() call runs the
// check-sso PKCE handshake, falls back to the saved session (sessionToken.ts)
// and returns a handle; islands/auth/store.ts owns the store/refresh-loop/UI.
// keycloak-js only loads when auth boots — never on the critical path.
//
// The login iframe is OFF. It reports "changed" the moment the SSO cookie is
// gone, which is exactly the state a restored session lives in, and it would
// clear the tokens every few seconds.

import type Keycloak from "keycloak-js";

import { forgetSession, refreshSession, restoreSession, saveSession } from "./sessionToken";

export interface AuthHandle {
  /** True when check-sso or the saved session adopted a session (or a login redirect landed). */
  authenticated: boolean;
  token: () => string | undefined;
  login: () => void;
  logout: (redirectUri: string) => void;
  /** Refresh when fewer than minValidity seconds remain; resolves false when still valid. */
  updateToken: (minValidity: number) => Promise<boolean>;
  /** Drop this tab's tokens, for a session another tab has ended. */
  clear: () => void;
  accountUrl: () => string;
}

export async function bootAuth(url: string, realm: string, clientId: string): Promise<AuthHandle> {
  const { default: KeycloakCtor } = await import("keycloak-js");
  const kc: Keycloak = new KeycloakCtor({ url, realm, clientId });
  const authenticated = await kc.init({
    onLoad: "check-sso",
    pkceMethod: "S256",
    silentCheckSsoRedirectUri: `${location.origin}/silent-check-sso.html`,
    checkLoginIframe: false,
  });
  if (authenticated) saveSession(kc);
  const restored = !authenticated && (await restoreSession(kc));
  return {
    authenticated: authenticated === true || restored,
    token: () => kc.token,
    login: () => void kc.login(),
    logout: (redirectUri: string) => {
      forgetSession();
      void kc.logout({ redirectUri });
    },
    updateToken: (minValidity: number) => refreshSession(kc, minValidity),
    clear: () => kc.clearToken(),
    accountUrl: () => kc.createAccountUrl(),
  };
}
