/**
 * `/account`: the identity card, then the danger zone — three permanent actions, each behind a
 * styled confirm modal, reporting through one inline status banner. The shared "account grammar"
 * the admin panel reuses. Deleting the account orchestrates erase → delete → sign-out ON THE
 * CLIENT (store.ts), so the server's identity context never depends on submissions. Anonymous
 * visitors see a sign-in prompt, not a broken page.
 */
import { render, h } from "preact";
import { useState } from "preact/hooks";

import { ApiFailure } from "../../lib/api/client";
import type { Me } from "../../lib/api/client";
import * as log from "../../lib/log";
import { useAuthState } from "./Chip";
import { deleteAccount, eraseAllData, eraseSubmissions, signIn } from "./store";

// ─────────────────────────────────────────────────────────────────────────────
// STATUS + THE PENDING ACTION
// ─────────────────────────────────────────────────────────────────────────────

type ActionStatus =
  | { kind: "idle" }
  | { kind: "busy"; message: string }
  | { kind: "ok"; message: string }
  | { kind: "error"; message: string };

/** Which destructive action awaits confirmation. */
type Pending = "erase" | "eraseAll" | "delete";

function failureMessage(error: unknown): string {
  return error instanceof ApiFailure ? error.message : error instanceof Error ? error.message : String(error);
}

// ─────────────────────────────────────────────────────────────────────────────
// THE PAGE
// ─────────────────────────────────────────────────────────────────────────────

export function AccountPanel() {
  const state = useAuthState();
  return (
    <div class="account-page">
      <div class="account-page__inner">
        <h1 class="account-page__title">Your account</h1>
        {state.kind === "loading" && <p class="account-page__loading">Loading…</p>}
        {state.kind === "anonymous" && (
          <div class="account-page__identity account-page__identity--anon">
            <p class="account-page__handle">Not signed in</p>
            <p class="account-page__meta">Sign in to run, submit, and manage your data.</p>
            <button class="account-page__signin" onClick={() => signIn()}>
              Sign in
            </button>
          </div>
        )}
        {state.kind === "authed" && <SignedIn me={state.me} />}
      </div>
    </div>
  );
}

function SignedIn({ me }: { me: Me }) {
  const [status, setStatus] = useState<ActionStatus>({ kind: "idle" });
  const [pending, setPending] = useState<Pending | null>(null);
  const avatar = me.username.charAt(0).toUpperCase();

  const run = (pendingAction: Pending) => {
    setPending(null);
    void (async () => {
      try {
        if (pendingAction === "erase") {
          setStatus({ kind: "busy", message: "Erasing your submissions…" });
          const deleted = await eraseSubmissions();
          setStatus({ kind: "ok", message: `Deleted ${deleted} submission(s).` });
        } else if (pendingAction === "eraseAll") {
          setStatus({ kind: "busy", message: "Erasing your data…" });
          await eraseAllData(); // reloads the page on success
        } else {
          setStatus({ kind: "busy", message: "Deleting your account…" });
          await deleteAccount(); // signs out on success
        }
      } catch (error) {
        setStatus({ kind: "error", message: failureMessage(error) });
      }
    })();
  };

  return (
    <>
      <div class="account-page__identity">
        <span class="account-page__avatar">{avatar}</span>
        <span class="account-page__id-text">
          <span class="account-page__handle">@{me.username}</span>
          {me.email && <span class="account-page__field">{me.email}</span>}
          <span class="account-page__meta">Signed in with Keycloak</span>
        </span>
        <a class="account-page__back" href="/">
          ← Back to the library
        </a>
      </div>
      <section class="account-page__danger">
        <p class="account-page__danger-head">
          <span class="account-page__danger-icon">⚠</span> Danger zone
        </p>
        <p class="account-page__danger-note">These actions are permanent and can't be undone.</p>
        <StatusBanner status={status} />
        <Card
          title="Erase my submissions"
          desc="Permanently delete every code submission you've made, across all problems."
          button="Erase"
          onClick={() => setPending("erase")}
        />
        <Card
          title="Erase all my data"
          desc="Erase your submissions and clear this browser's reading preferences. Reloads the page."
          button="Erase everything"
          onClick={() => setPending("eraseAll")}
        />
        <Card
          title="Delete my account"
          desc="Erase your submissions and permanently remove your sign-in. You'll be signed out."
          button="Delete account"
          onClick={() => setPending("delete")}
        />
      </section>
      {pending && <ConfirmModal pending={pending} onCancel={() => setPending(null)} onConfirm={run} />}
    </>
  );
}

function Card({
  title,
  desc,
  button,
  onClick,
}: {
  title: string;
  desc: string;
  button: string;
  onClick: () => void;
}) {
  return (
    <div class="account-page__card">
      <span class="account-page__card-text">
        <span class="account-page__card-title">{title}</span>
        <span class="account-page__card-desc">{desc}</span>
      </span>
      <button class="account-page__btn account-page__btn--danger" onClick={onClick}>
        <span class="account-page__btn-icon">🗑</span>
        {button}
      </button>
    </div>
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

/** The styled stand-in for `window.confirm` — Cancel or the named danger verb; Esc / scrim closes. */
function ConfirmModal({
  pending,
  onCancel,
  onConfirm,
}: {
  pending: Pending;
  onCancel: () => void;
  onConfirm: (pending: Pending) => void;
}) {
  const copy = CONFIRM_COPY[pending];
  return (
    <div
      class="confirm-scrim"
      onClick={(event) => {
        if (event.target === event.currentTarget) onCancel();
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") onCancel();
      }}
    >
      <div class="confirm">
        <p class="confirm__title">{copy.title}</p>
        <p class="confirm__body">{copy.body}</p>
        <div class="confirm__actions">
          <button class="confirm__cancel" onClick={onCancel}>
            Cancel
          </button>
          <button class="confirm__danger" onClick={() => onConfirm(pending)}>
            {copy.verb}
          </button>
        </div>
      </div>
    </div>
  );
}

const CONFIRM_COPY: Record<Pending, { title: string; body: string; verb: string }> = {
  erase: {
    title: "Erase your submissions?",
    body: "Every attempt you've saved will be deleted. This can't be undone.",
    verb: "Erase",
  },
  eraseAll: {
    title: "Erase all your data?",
    body: "Your submissions and this browser's reading preferences will be gone.",
    verb: "Erase everything",
  },
  delete: {
    title: "Delete your account?",
    body: "Your submissions and your sign-in will be permanently removed.",
    verb: "Delete account",
  },
};

// ── mount (self-hydrating, the pattern islands/workbench/index.ts sets) ────────────────────────
const root = document.querySelector<HTMLElement>("[data-account-root]");
if (root) {
  render(h(AccountPanel, {}), root);
  log.info("account page mounted");
}
