/**
 * `/admin`: the two allowlists and the content repositories. `me.admin` only gates the UI; every
 * call is re-checked
 * server-side, so a non-admin who navigates here just sees the API's 403 in each section's banner.
 * Anonymous / non-admin see "Admin only".
 *
 * The two lists are DELIBERATELY separate. The submit allowlist grants shared compute and storage
 * for saving code attempts; the content-editor list grants the ability to open pull requests
 * against the content repository under this deployment's token — a much larger trust grant.
 * Revoking one must never be the same act as revoking the other. They share a table/form/banner
 * only because they share a shape, not a meaning — see AllowlistSection.
 */
import { render, h } from "preact";

import {
  allowlist,
  allowlistGrant,
  allowlistRevoke,
  contentEditors,
  contentEditorGrant,
  contentEditorRevoke,
} from "../../lib/api/client";
import * as log from "../../lib/log";
import { AllowlistSection } from "./AllowlistSection";
import { ContentSourcesSection } from "./ContentSourcesSection";
import { useAuthState } from "./Chip";
import { signIn } from "./store";

export function AdminPanel() {
  const state = useAuthState();
  return (
    <div class="account-page">
      <div class="account-page__inner">
        <h1 class="account-page__title">Admin</h1>
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
        {state.kind === "authed" && state.me.admin && (
          <>
            <AllowlistSection
              title="Submit allowlist"
              blurb="Who may SAVE code attempts when the allowlist is enforced. Usernames are stored lowercase."
              load={allowlist}
              grant={allowlistGrant}
              revoke={allowlistRevoke}
            />
            <AllowlistSection
              title="Content editors"
              blurb="Who may propose prose edits from the in-app editor. A grant here opens pull requests against the content repository under this deployment's token — a larger trust grant than saving attempts, which is why it is a separate list."
              load={contentEditors}
              grant={contentEditorGrant}
              revoke={contentEditorRevoke}
            />
            <ContentSourcesSection />
          </>
        )}
      </div>
    </div>
  );
}

// ── mount (self-hydrating) ────────────────────────────────────────────────────────────────────
const root = document.querySelector<HTMLElement>("[data-admin-root]");
if (root) {
  render(h(AdminPanel, {}), root);
  log.info("admin page mounted");
}
