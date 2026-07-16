# Step 17 — Identity, client side: PKCE boot, the account chip, and the Edit gate

*(oracle: synapse step 17's client half — `AuthBoot`/`AuthStore`, the keycloak-js island, the
account chip, bearer injection into every API call, and the auth-gated Edit; the identity
feature is born on the three-layer split as the oracle's post-refactor final design.)*

## The island (`islands/auth/loader.ts` + `src/islands/auth.rs`)

keycloak-js stays TS behind the narrow seam the other islands use: `bootAuth(url, realm,
clientId)` dynamic-imports the library (its chunk loads only when auth boots), runs
`init({onLoad: "check-sso", pkceMethod: "S256", silentCheckSsoRedirectUri})` against
`public/silent-check-sso.html` (one `postMessage` line — the iframe echo the adapter listens
for), and hands back a plain handle: `authenticated · token() · login() · logout(redirect) ·
updateToken(min) · accountUrl()`. The Rust side is a `#[wasm_bindgen(module = "@auth/loader")]`
extern over that handle — no keycloak types cross the bridge.

## The store (`identity/state`)

`AuthStatus = Loading | Anonymous | Authed(MeDto)` — starting `Loading` so the header never
flashes "Sign in" before check-sso answers. `AuthStore::provide()` runs once in `App`: it
installs the **token-provider seam** (`api::set_token_provider` — a thread-local `fn()` the
fetch helpers consult, so `api/` stays feature-agnostic with an anonymous default) and spawns
the boot flow: `GET /api/auth/config` → island boot → if authenticated, **adopt** the session
by echoing `GET /api/me` (the session exists only once OUR server verifies the token) → a 30 s
poll refreshing when < 60 s remain. Every failure lands on `Anonymous`, never an error page.
The live JS handle is a session-scoped `thread_local` (`Rc<AuthHandle>`, `!Send` — the same
ownership shape as the mounted editors).

## The chip and the gates

`AccountChip` sits right of a `shell-spacer` in the header: Loading → a quiet "…", Anonymous →
"Sign in" (`kc.login()` — the full PKCE redirect), Authed → "@username" opening a two-item menu
(Manage account → `kc.createAccountUrl()`, Sign out → `kc.logout(origin)`). The workbench Edit
button gets the oracle's gate: `disabled` while anonymous with the tip "Sign in to edit this
code", and the ⌘E keymap path checks the same signal so the shortcut can't bypass the button.

**The architecture bug this step surfaced:** runnable blocks mount OUT-OF-TREE
(`leptos::mount::mount_to` starts a fresh root owner), so `AuthStore::from_context()` panicked
inside them — App's context is unreachable from a hydrated mount. The fix is the honest one:
the reader captures the store **in-tree** and threads it as a prop through
`hydrate_workbenches → RunnableBlock`. Context is a tree feature; hydrated islands take props.

## Verified live (compose Keycloak 26, realm `synapse`)

Anonymous: chip "Sign in", Edit disabled with the tip. Sign in → the real Keycloak login →
redirect back → chip `@tester`, Edit enabled. Submit while authed stores the verified `sub`
on the row (checked in Postgres) and judges normally. Menu → Sign out → redirect home, chip
back to "Sign in". Suite: 159 Rust + 40 vitest; critical path 332 KiB gz (keycloak-js rides a
lazy chunk).

Next: blog, ⌘K search, rate limiting, and the SPA + `/c4` proxy fallthrough.
