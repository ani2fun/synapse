/**
 * The auth store + boot flow (oracle: client/src/identity/state/mod.rs — `AuthStore` + `AuthBoot`).
 *
 * A Leptos `RwSignal` in the old client; islands cannot share a signal, so the singleton lives
 * here as a plain module (Vite dedupes it into one shared chunk, so the header chip, the account
 * page and the admin page all observe the SAME instance) with a subscribe/notify seam plus the
 * window-scoped contracts the workbench/problem gates read. The state starts `loading` — never a
 * "Sign in" flash before check-sso answers — adopts the session by echoing `GET /api/me` (the
 * SERVER, not the token, is the identity authority), and a 30 s loop refreshes the token when
 * < 60 s remain; a failed refresh degrades to `anonymous`. Every failure lands on `anonymous`,
 * never an error page. NEVER logs a token.
 */

import * as api from "../../lib/api/client";
import type { Me } from "../../lib/api/client";
import * as log from "../../lib/log";
import * as storage from "../../lib/storage";
import type { AuthHandle } from "../../lib/islands/auth/loader";
import { bootAuth } from "../../lib/islands/auth/loader";
import { AUTH_CHANGED } from "../workbench/contracts";

// ─────────────────────────────────────────────────────────────────────────────
// STATE
// ─────────────────────────────────────────────────────────────────────────────

export type AuthState =
  | { readonly kind: "loading" }
  | { readonly kind: "anonymous" }
  | { readonly kind: "authed"; readonly me: Me };

let state: AuthState = { kind: "loading" };
/** The live keycloak handle — session-scoped, owned here (a JS object). */
let handle: AuthHandle | null = null;
let booted = false;
let seamsInstalled = false;

const listeners = new Set<() => void>();

export function getState(): AuthState {
  return state;
}

/** Subscribe to state flips; returns an unsubscribe. Preact views use it to re-render. */
export function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The current bearer, or null when anonymous — read afresh per call (a refresh needs no
 *  re-install). The api client and the viz wasm both read the token through this. */
function currentToken(): string | null {
  return handle?.token() ?? null;
}

/** The bearer seam + the workbench/viz window contracts. Installed ONCE, before the first
 *  `/api/me` — the getters read the live `handle`/`state`, so a token refresh needs no re-install
 *  (the oracle installs the provider once in `AuthStore::provide`). */
function installSeams(): void {
  if (seamsInstalled) return;
  seamsInstalled = true;
  api.installTokenProvider(currentToken);
  // The viz wasm's bearer seam (contracts.ts) — identity and the lazy wasm load in either order.
  window.__synapseVizToken = currentToken;
  // The workbench/problem gates read this at render time (contracts.ts).
  window.__synapseAuth = () => state.kind === "authed";
}

/** Flip the state: notify Preact subscribers AND dispatch AUTH_CHANGED so already-mounted
 *  workbenches / problem pages re-render their Edit/Submit gates. */
function setState(next: AuthState): void {
  state = next;
  for (const listener of listeners) listener();
  window.dispatchEvent(new Event(AUTH_CHANGED));
}

export function isAuthed(): boolean {
  return state.kind === "authed";
}

// ─────────────────────────────────────────────────────────────────────────────
// BOOT (oracle: AuthBoot.start) — config → check-sso (PKCE S256) → adopt → refresh loop
// ─────────────────────────────────────────────────────────────────────────────

/** Idempotent: the header island calls this on every page; it runs once per document. */
export async function boot(): Promise<void> {
  if (booted) return;
  booted = true;
  installSeams();

  let config: api.AuthConfig;
  try {
    config = await api.authConfig();
  } catch {
    log.warn("auth: config unavailable — staying anonymous");
    setState({ kind: "anonymous" });
    return;
  }
  log.debug(`auth: config realm '${config.realm}' at ${config.url} (client ${config.clientId})`);

  try {
    handle = await bootAuth(config.url, config.realm, config.clientId);
  } catch (error) {
    log.warn(`auth: boot failed — staying anonymous (${describe(error)})`);
    setState({ kind: "anonymous" });
    return;
  }

  if (!handle.authenticated) {
    log.info("auth: check-sso → anonymous");
    setState({ kind: "anonymous" });
    return;
  }
  await adopt();
  void refreshLoop();
}

/** The session is adopted only when OUR server verifies the token (`/api/me`). */
async function adopt(): Promise<void> {
  try {
    const me = await api.me();
    setState({ kind: "authed", me });
    log.info(`auth: adopted @${me.username}${me.admin ? " (admin)" : ""}`);
  } catch {
    log.info("auth: check-sso → anonymous (server did not adopt the token)");
    setState({ kind: "anonymous" });
  }
}

/** Poll every 30 s, refreshing when < 60 s remain; a failed refresh means the session is gone
 *  (the oracle polls `updateToken`, not `onTokenExpired`). */
async function refreshLoop(): Promise<void> {
  for (;;) {
    await sleep(30_000);
    if (state.kind !== "authed") return;
    try {
      await handle?.updateToken(60);
    } catch {
      log.warn("auth: session refresh failed — signed out");
      setState({ kind: "anonymous" });
      return;
    }
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// SESSION ACTIONS (oracle: AuthStore::sign_in / sign_out / account_url)
// ─────────────────────────────────────────────────────────────────────────────

export function signIn(): void {
  handle?.login();
}

export function signOut(): void {
  handle?.logout(window.location.origin);
  setState({ kind: "anonymous" });
  log.info("auth: signed out");
}

export function accountUrl(): string | null {
  return handle?.accountUrl() ?? null;
}

// ─────────────────────────────────────────────────────────────────────────────
// ACCOUNT ACTIONS (oracle: state/mod.rs erase_submissions / erase_all_data / delete_account)
// Deleting the account orchestrates erase → delete → sign-out ON THE CLIENT, so the server's
// identity context never depends on submissions (step-20 design).
// ─────────────────────────────────────────────────────────────────────────────

/** The browser-side state the account owns — reading preferences + progress. Theme is a
 *  preference of the DEVICE, not "my data" (the oracle excluded it too). */
const LOCAL_KEYS = [
  storage.READER_PREFS_KEY,
  storage.READER_PROGRESS_KEY,
  storage.READER_LAST_KEY,
] as const;

function clearReaderStorage(): void {
  for (const key of LOCAL_KEYS) storage.remove(key);
}

/** Erase every submission of the caller ("reset my data"). Returns the deleted count. */
export async function eraseSubmissions(): Promise<number> {
  const result = await api.eraseSubmissions();
  return result.deleted;
}

/** Erase server data AND this browser's reading state, then reload. */
export async function eraseAllData(): Promise<void> {
  await api.eraseSubmissions();
  clearReaderStorage();
  window.location.reload();
}

/** Erase → delete → sign out; any failed leg throws before the next, stopping the chain. */
export async function deleteAccount(): Promise<void> {
  await api.eraseSubmissions();
  await api.deleteMe();
  clearReaderStorage();
  signOut();
}

// ─────────────────────────────────────────────────────────────────────────────
// HELPERS
// ─────────────────────────────────────────────────────────────────────────────

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
