# A11 — Identity: the auth island, account, admin

The last window contract gets its writer. Since A06 every gate in the Astro app has read
`window.__synapseAuth` and listened for `AUTH_CHANGED`, rendering the honest anonymous
experience; A11 installs the real thing — and not one line of the workbench, problem page, or
editorial changed when it arrived. That was the point of declaring the contracts in one file
five steps early.

## The boot flow (islands/auth/store.ts, 212 lines)

A port of the old `AuthStore` (state/mod.rs): fetch `/api/auth/config` → lazy-import the
keycloak-js loader → `check-sso` with PKCE S256 and the silent-check-sso iframe → on a token,
adopt the session via `GET /api/me` — the SERVER is the identity authority, never the token's
own claims (the step-16 rule, carried over) → 30s refresh loop. Every state flip installs the
seams in one place (`installSeams`): the TS api client's `installTokenProvider`, the viz wasm's
`window.__synapseVizToken` (read per-request by the crate, so refresh needs no re-install —
A10's design paying off), `window.__synapseAuth`, and an `AUTH_CHANGED` dispatch. Any boot
failure — config unreachable, Keycloak down, origin not allowlisted — degrades to Anonymous
with a WARN, never a crash. Tokens never appear in a log line.

## The chrome and the pages

The header chip (Preact, 67 lines) keeps the oracle's three faces: Loading is a QUIET
placeholder (no "Sign in" flash before check-sso answers), Anonymous offers sign-in, Authed
shows @username with Manage account & data / Admin panel (admin only, UX-gated — the server
re-checks per call) / Sign out. `/account` (AccountPanel, 231 lines) ports the step-20 grammar:
identity card, danger zone × 3, confirm modal, status banner — the CLIENT orchestrates
erase→delete→sign-out, and an erase also clears the reader's localStorage keys. `/admin`
(AdminPanel, 203 lines) ports the step-21 allowlist panel: table, grant form (canonicalised
server-side), revoke, "Admin only" for everyone else. Zero api-client additions — every
endpoint already existed from A02's full-surface port.

## Single-sourcing

`git mv client/islands/auth → web/src/lib/islands/auth`; the old client's `@auth` alias
repoints (the @editor/@tracer/@diagram pattern), and its build stays green — proven.
`silent-check-sso.html` is deliberately DUPLICATED into `web/public/` (not moved): the old
client's Vite ships its own until A14. keycloak-js stays a lazy split chunk.

## Verified live (real Keycloak 26, dev realm, prod-shaped serve on :8280)

The full PKCE round trip headless: Sign in → Keycloak form (tester) → redirect → chip
`@tester`, log `auth: adopted @tester (admin)`; `__synapseAuth()` true and `__synapseVizToken()`
returning a real JWT. The problem page's Edit AND Submit enabled live, the anonymous sign-in
bar gone. An authed submit carried `Authorization: Bearer …` on the wire (request-intercepted,
not inferred) and reached Judging. `/account` rendered the identity card; `/admin` rendered the
seeded allowlist (test1). Sign out returned both seams to anonymous (gate false, token null)
and the chip to Sign in. One environmental note for future sessions: the PKCE loop only
completes on an origin in the Keycloak client's allowlist (:5373/:8280) — on any other port
check-sso is origin-rejected and the boot takes its designed anonymous path, which looks like
a bug if you forget this.

## Numbers

186 web + 18 client vitest · 7/7 e2e · new: store 212, chip 67, account 231, admin 203,
pages 17+18 · keycloak-js lazy chunk · zero api-client endpoint additions.

## The lesson

**Install the reader before the writer, and the writer ships without friction.** Five steps of
"reads anonymous, correctly" meant identity landed as pure addition: the gates flipped live on
an event that had been dispatched into the void since A06. The mirror lesson is environmental:
a security feature's failure mode is designed to be quiet (stay anonymous, warn, never crash) —
so verification MUST distinguish "gracefully anonymous because the origin isn't allowlisted"
from "signed in"; only the allowlisted origin proves the loop.
