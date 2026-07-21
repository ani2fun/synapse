/**
 * The header account chip (oracle: client/src/identity/view/mod.rs — `AccountChip`).
 * `loading` renders a QUIET placeholder (no "Sign in" flash before check-sso answers);
 * `anonymous` offers sign-in; `authed` shows @username with a menu — Manage account & data,
 * Admin panel (only when `me.admin`; UX-only, the server re-checks per call), Sign out.
 * Classes are the old chip's `account-chip*` (client/styles/reader.css), byte-faithful.
 */
import { useEffect, useState } from "preact/hooks";

import { getState, signIn, signOut, subscribe } from "./store";
import type { AuthState } from "./store";

/** Re-render on every store flip — the same shape as the Rust chip reading its context signal. */
export function useAuthState(): AuthState {
  const [snapshot, setSnapshot] = useState<AuthState>(getState());
  useEffect(() => subscribe(() => setSnapshot(getState())), []);
  return snapshot;
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
