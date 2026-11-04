// ──────────────────────────────────────────────────────────────────
// THE SAVED SESSION — a refresh token that outlives the browser
// ──────────────────────────────────────────────────────────────────
// Keycloak's SSO cookie has no expiry for a GitHub login, so quitting the browser loses it and
// check-sso answers "anonymous" while the 30-day session is still alive on the server. The refresh
// token kept here is what picks that session back up on the next page.
//
// The realm ROTATES refresh tokens, and using one too often ends the session outright. So every
// refresh runs under one lock shared by the tabs, and starts from whatever token is saved: a tab
// holding the token another tab has since replaced would otherwise spend it. NEVER log a token.

import * as storage from "../../storage";

/** The part of keycloak-js this module drives. */
export interface RefreshClient {
  refreshToken?: string | undefined;
  updateToken(minValidity: number): Promise<boolean>;
}

/** A forced refresh: spend the token whatever the access token's remaining life. */
const FORCE = -1;
const LOCK = "synapse-auth-refresh";

function exclusive<T>(task: () => Promise<T>): Promise<T> {
  // Absent outside a secure context, where a second tab's refresh is the risk accepted.
  return navigator.locks ? navigator.locks.request(LOCK, task) : task();
}

/** Keycloak answered and said no — the token or its session is over. A network failure or a 5xx is
 *  not this: the session may be fine, so it must not cost the reader their sign-in. */
export function isRejected(error: unknown): boolean {
  return (error as { response?: { status?: number } } | null)?.response?.status === 400;
}

/** Keep the client's current refresh token for the next page. */
export function saveSession(client: RefreshClient): void {
  if (client.refreshToken) storage.set(storage.AUTH_REFRESH_KEY, client.refreshToken);
}

export function forgetSession(): void {
  storage.remove(storage.AUTH_REFRESH_KEY);
}

async function refreshFromSaved(client: RefreshClient, minValidity: number): Promise<boolean> {
  const saved = storage.get(storage.AUTH_REFRESH_KEY);
  if (saved) client.refreshToken = saved;
  try {
    const refreshed = await client.updateToken(minValidity);
    saveSession(client);
    return refreshed;
  } catch (error) {
    if (isRejected(error)) forgetSession();
    throw error;
  }
}

/** Refresh when fewer than `minValidity` seconds remain; resolves false when still valid. Rejects
 *  when the refresh fails — `isRejected` says whether the session is over. */
export function refreshSession(client: RefreshClient, minValidity: number): Promise<boolean> {
  return exclusive(() => refreshFromSaved(client, minValidity));
}

/** Pick up a saved session on a client check-sso found no session for. True when it is signed in. */
export function restoreSession(client: RefreshClient): Promise<boolean> {
  return exclusive(async () => {
    if (!storage.get(storage.AUTH_REFRESH_KEY)) return false;
    try {
      await refreshFromSaved(client, FORCE);
      return true;
    } catch {
      delete client.refreshToken;
      return false;
    }
  });
}
