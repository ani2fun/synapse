/**
 * The header account chip. `loading` renders a QUIET placeholder (no "Sign in" flash before
 * check-sso answers); `anonymous` offers sign-in; `authed` shows @username with a menu — Manage
 * account & data, Admin panel (only when `me.admin`; UX-only, the server re-checks per call),
 * Sign out. Classes are `account-chip*` (web/styles/reader.css).
 */
import { useState } from "preact/hooks";
import { useSyncExternalStore } from "preact/compat";

import { getState, signIn, signOut, subscribe } from "./store";
import type { AuthState } from "./store";

/**
 * Re-render on every store flip.
 *
 * `useSyncExternalStore` rather than a `useState` snapshot plus a `useEffect` subscription, for
 * one reason: it RE-READS the store after subscribing. The hand-rolled pair cannot — it captures
 * the state at mount and only registers its listener after first paint, so a flip landing in that
 * window is delivered to nobody and the chip sits on its placeholder for the rest of the session.
 * That is not hypothetical: `check-sso` answers in microseconds when Keycloak is unreachable, and
 * it beat the effect every time — the store logged `anonymous` while the header still showed `…`.
 * The primitive exists to close exactly this gap; `lib/store.ts` reaches for it for the same
 * reason. `getState` returns the singleton, whose identity changes only when `setState` assigns,
 * so the snapshot is stable and this never spins.
 */
export function useAuthState(): AuthState {
  return useSyncExternalStore(subscribe, getState);
}

export function Chip() {
  const state = useAuthState();
  const [open, setOpen] = useState(false);

  if (state.kind === "loading") {
    return (
      <span class="account-chip">
        <span class="account-chip__quiet">…</span>
      </span>
    );
  }

  if (state.kind === "anonymous") {
    return (
      <span class="account-chip">
        <button class="account-chip__signin" onClick={() => signIn()}>
          Sign in
        </button>
      </span>
    );
  }

  const me = state.me;
  return (
    <span class="account-chip">
      <span class="account-chip__menu-wrap">
        <button class="account-chip__user" onClick={() => setOpen((o) => !o)}>
          @{me.username}
        </button>
        {open && (
          <span class="account-chip__menu">
            <a class="account-chip__item" href="/account" onClick={() => setOpen(false)}>
              Manage account &amp; data
            </a>
            {me.admin && (
              <a class="account-chip__item" href="/admin" onClick={() => setOpen(false)}>
                Admin panel
              </a>
            )}
            <button class="account-chip__item" onClick={() => signOut()}>
              Sign out
            </button>
          </span>
        )}
      </span>
    </span>
  );
}
