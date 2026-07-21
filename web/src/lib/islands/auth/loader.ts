// ──────────────────────────────────────────────────────────────────
// AUTH LOADER — keycloak-js behind the @auth lazy chunk
// ──────────────────────────────────────────────────────────────────
// The oracle's KeycloakJs facade + AuthBoot init, flattened to the
// wasm-friendly FFI shape (same adaptation as @markdown/@editor): one
// bootAuth() call runs the check-sso PKCE handshake and returns a handle;
// the wasm side owns the store/refresh-loop/UI. keycloak-js only loads
// when auth boots — never on the critical path.

import type Keycloak from "keycloak-js";

export interface AuthHandle {
  /** True when check-sso adopted an existing session (or a login redirect landed). */
  authenticated: boolean;
  token: () => string | undefined;
  login: () => void;
  logout: (redirectUri: string) => void;
  /** Refresh when fewer than minValidity seconds remain; resolves false when still valid. */
  updateToken: (minValidity: number) => Promise<boolean>;
  accountUrl: () => string;
}

export async function bootAuth(url: string, realm: string, clientId: string): Promise<AuthHandle> {
  const { default: KeycloakCtor } = await import("keycloak-js");
  const kc: Keycloak = new KeycloakCtor({ url, realm, clientId });
  const authenticated = await kc.init({
    onLoad: "check-sso",
    pkceMethod: "S256",
    silentCheckSsoRedirectUri: `${location.origin}/silent-check-sso.html`,
  });
  return {
    authenticated: authenticated === true,
    token: () => kc.token,
    login: () => void kc.login(),
    logout: (redirectUri: string) => void kc.logout({ redirectUri }),
    updateToken: (minValidity: number) => kc.updateToken(minValidity),
    accountUrl: () => kc.createAccountUrl(),
  };
}
