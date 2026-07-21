/**
 * `/admin` (oracle: client/src/identity/view/admin_page.rs — `AdminPage`, step 35): the submit
 * allowlist panel on the account grammar — the grants table, the grant form, revoke per row,
 * one status banner. `me.admin` only gates the UI; every call is re-checked server-side, so a
 * non-admin who navigates here just sees the API's 403 in the banner. Anonymous / non-admin see
 * "Admin only".
 */
import { render, h } from "preact";
import { useEffect, useState } from "preact/hooks";

import { allowlist, allowlistGrant, allowlistRevoke, ApiFailure } from "../../lib/api/client";
import type { AllowlistEntry } from "../../lib/api/client";
import * as log from "../../lib/log";
import { useAuthState } from "./Chip";
import { signIn } from "./store";

type ActionStatus =
  | { kind: "idle" }
  | { kind: "busy"; message: string }
  | { kind: "ok"; message: string }
  | { kind: "error"; message: string };

type Entries =
  | { kind: "loading" }
  | { kind: "loaded"; rows: AllowlistEntry[] }
  | { kind: "failed"; message: string };

function failureMessage(error: unknown): string {
  return error instanceof ApiFailure ? error.message : error instanceof Error ? error.message : String(error);
}

// ─────────────────────────────────────────────────────────────────────────────
// THE PAGE
// ─────────────────────────────────────────────────────────────────────────────

export function AdminPanel() {
  const state = useAuthState();
  return (
    <div class="account-page">
      <div class="account-page__inner">
        <h1 class="account-page__title">Admin — submit allowlist</h1>
        {state.kind === "loading" && <p class="account-page__loading">Loading…</p>}
        {state.kind === "anonymous" && (
          <div class="account-page__identity account-page__identity--anon">
            <p class="account-page__handle">Not signed in</p>
            <button class="account-page__signin" onClick={() => signIn()}>
              Sign in
            </button>
          </div>
        )}
        {state.kind === "authed" && !state.me.admin && (
          <div class="account-page__identity account-page__identity--anon">
            <p class="account-page__handle">Admin only</p>
            <p class="account-page__meta">This deployment doesn't list you as an admin.</p>
          </div>
        )}
        {state.kind === "authed" && state.me.admin && <Panel />}
      </div>
    </div>
  );
}

function Panel() {
  const [status, setStatus] = useState<ActionStatus>({ kind: "idle" });
  const [entries, setEntries] = useState<Entries>({ kind: "loading" });
  const [username, setUsername] = useState("");
  const [note, setNote] = useState("");

  const reload = () => {
    void (async () => {
      try {
        const rows = await allowlist();
        setEntries({ kind: "loaded", rows });
      } catch (error) {
        setEntries({ kind: "failed", message: failureMessage(error) });
      }
    })();
  };

  useEffect(reload, []);

  const grant = () => {
    if (username.trim() === "") {
      setStatus({ kind: "error", message: "A grant needs a username" });
      return;
    }
    const request = { username, note: note.trim() === "" ? null : note };
    setStatus({ kind: "busy", message: "Granting…" });
    void (async () => {
      try {
        const entry = await allowlistGrant(request);
        setStatus({ kind: "ok", message: `Granted '${entry.username}'.` });
        setUsername("");
        setNote("");
        reload();
      } catch (error) {
        setStatus({ kind: "error", message: failureMessage(error) });
      }
    })();
  };

  const revoke = (name: string) => {
    setStatus({ kind: "busy", message: `Revoking '${name}'…` });
    void (async () => {
      try {
        await allowlistRevoke(name);
        setStatus({ kind: "ok", message: `Revoked '${name}'.` });
        reload();
      } catch (error) {
        setStatus({ kind: "error", message: failureMessage(error) });
      }
    })();
  };

  return (
    <>
      <p class="account-page__meta">
        Who may SAVE attempts when the allowlist is enforced. Usernames are stored lowercase.
      </p>
      <StatusBanner status={status} />
      <form
        class="admin__grant"
        onSubmit={(event) => {
          event.preventDefault();
          grant();
        }}
      >
        <input
          class="admin__input"
          placeholder="username"
          value={username}
          onInput={(event) => setUsername((event.target as HTMLInputElement).value)}
        />
        <input
          class="admin__input admin__input--note"
          placeholder="note (optional)"
          value={note}
          onInput={(event) => setNote((event.target as HTMLInputElement).value)}
        />
        <button class="admin__grant-btn" type="submit">
          Grant
        </button>
      </form>
      <EntriesTable entries={entries} revoke={revoke} />
    </>
  );
}

function EntriesTable({ entries, revoke }: { entries: Entries; revoke: (name: string) => void }) {
  if (entries.kind === "loading") return <p class="account-page__loading">Loading grants…</p>;
  if (entries.kind === "failed")
    return <p class="account-page__status account-page__status--error">{entries.message}</p>;
  if (entries.rows.length === 0) return <p class="account-page__meta">No grants yet.</p>;
  return (
    <table class="admin__table">
      <thead>
        <tr>
          <th>Username</th>
          <th>Note</th>
          <th>Granted</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {entries.rows.map((entry) => (
          <tr key={entry.username}>
            <td class="admin__cell-user">{entry.username}</td>
            <td>{entry.note ?? ""}</td>
            <td>{entry.grantedAt.split("T")[0] ?? ""}</td>
            <td>
              <button class="admin__revoke" onClick={() => revoke(entry.username)}>
                Revoke
              </button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function StatusBanner({ status }: { status: ActionStatus }) {
  if (status.kind === "idle") return null;
  const cls =
    status.kind === "busy"
      ? "account-page__status account-page__status--busy"
      : status.kind === "ok"
        ? "account-page__status account-page__status--ok"
        : "account-page__status account-page__status--error";
  const icon = status.kind === "busy" ? "…" : status.kind === "ok" ? "✓" : "✗";
  return (
    <p class={cls}>
      <span class="account-page__status-icon">{icon}</span> {status.message}
    </p>
  );
}

// ── mount (self-hydrating) ────────────────────────────────────────────────────────────────────
const root = document.querySelector<HTMLElement>("[data-admin-root]");
if (root) {
  render(h(AdminPanel, {}), root);
  log.info("admin page mounted");
}
