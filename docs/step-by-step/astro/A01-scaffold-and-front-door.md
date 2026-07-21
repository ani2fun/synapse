# A01 — The scaffold and the front door

*(the whole migration's rollback story is one env var, so that claim gets built — and tested — first.)*

> Branch chapter: the Astro migration runs on branch `astro`, numbered A01–A14, folded into the
> main ledger at merge. Main is at step-65 and keeps moving underneath.

## What this step is for

Nothing in A01 is user-visible. Its job is to make every later step cheap and reversible:
`web/` exists and SSRs one real page, the axum front door can route pages to it behind an env
var, every gate covers the new tree, and the branch has CI. The migration proper starts at A02;
this step is the ground it stands on.

## The front door, and why it is a fallback

`astro_proxy` is modelled on `likec4_proxy` — buffered, GET-shaped, 502 on an unreachable
upstream — but mounted as the router's **fallback**, not a wildcard route. Registered routes
(`/api`, `/media`, `/c4`, robots, sitemap) always beat the fallback in axum, so the
Cortex-inherited "greedy wildcard shadows /api" scar cannot recur *by construction*, and the
sidecar's own 404 page becomes the site 404.

The header contract is written once, in the module doc, and tested per clause:

- **Upstream:** path+query as received; `accept` and `if-none-match` forwarded;
  `accept-encoding` **stripped** (the response re-enters axum's CompressionLayer — compressing
  on both sides would either double-compress or make axum ship bytes it cannot inspect);
  `authorization` and `cookie` **never cross** — SSR renders anonymous by design, auth is a
  browser island, and the sidecar has no business seeing credentials.
- **Back:** status + `content-type`, `cache-control`, `etag`, `vary`, `location`. Everything
  else the sidecar says is dropped — the axum stack owns security headers, compression and
  tracing. `/_astro/*` hashed assets get `immutable` stamped if the sidecar omitted a cache
  header, matching the old client's policy.

`AppDeps.astro_url: Option<String>` (`SYNAPSE_ASTRO_URL`) picks the front end: `Some` = proxy
fallback, `None` = yesterday's `StaticRoutes`, byte for byte. That equivalence is a test, not a
promise — the whole rollback story rests on it.

## Robots and sitemap moved out first

They were inside `StaticRoutes`, which would have made crawler plumbing change identity with
the front end. They are generated from the in-memory catalog index, and the catalog lives in
the axum process whichever side serves pages — so they moved to `platform/seo_routes.rs` and
mount **unconditionally**, before either front end. Post-migration they stay axum-side for the
same reason: shipping the catalog across a process boundary to format 237 `<loc>` lines would
be architecture in the wrong direction.

## The scaffold

`web/` is Astro 5, `output: "server"`, the Node standalone adapter, and the Preact integration
(for the four stateful islands, per the plan). `Base.astro` carries the theme bootstrap
**verbatim** from `client/index.html` — same inline script, same pre-paint `.dark` semantics —
and imports `tokens.css`/`shell.css` **from `client/styles/`**: the stylesheets are
single-sourced until A14 moves them, so there is no drift window. The head is native props;
step 50's string-surgery machinery is not needed on any page Astro serves.

Dev loop: `dev-tools/dev` — axum on :8280 in the background, `astro dev` on **:5373**
foreground with HMR, proxying `/api`/`/media`/`/c4`. :5373 because the Keycloak dev realm
allowlists that origin; a silent port bump 403s the silent-SSO iframe (the step-39 scar). Port
reclamation via the shared `lib-ports.sh`.

*(Naming, revised after a real confusion: this script began life as `dev-astro` beside the old
`dev`. Both loops reclaim the same ports, so starting the old one silently killed the Astro
processes and served the Leptos client on :5373 — which is byte-identical to main's and read as
"the branch is running main's code". The Astro loop is now `dev`, the old one is `dev-leptos`
until A14 removes it, and both print a banner naming the client they serve, the branch, and the
ports.)*

## What the placeholder page caught immediately

`index.astro` SSR-fetches `/api/synapse/index` and lists the books — a throwaway A04 replaces.
Its first draft guessed the wire shape from field presence (`"categoryPath" in entry`) and
**silently rendered one book of seven**. The index is kind-discriminated (`kind: "book" |
"category"` — step 06's design); the guess was wrong in a way no compiler could see.

That is the argument for A02 in one incident: hand-written TS types against a wire contract the
server already publishes as OpenAPI are guesses with syntax highlighting. The fix here reads
`kind`; the systemic fix is the generated `schema.gen.ts` next step.

## Gates

- `check-conventions.sh` caps every `.ts`/`.tsx`/`.astro` under `web/` at 800 lines, with its
  own `web_ok` flag and `if [[ -d web ]]` guard so it survives `client/`'s deletion in A14.
  **Bite-tested** with a 900-line file — and the first bite-test read `$?` after piping through
  `tail`, which is the step-52 pipefail scar in miniature; re-verified unpiped, exit 1.
- CI gains a `web` job (npm ci → vitest `--passWithNoTests` until A02 → `astro build`), 15-min
  budget, `needs: conventions`. `astro build` needs no running API — output is `server`, pages
  fetch at request time, the build only bundles.
- Seven new ITs in `astro_proxy_it.rs` drive the FULL `app()` against a real stub sidecar:
  credentials never cross, accept-encoding stripped, etag/cache-control copied back, security
  headers stamped on proxied pages, `/api` never proxied, robots+sitemap serve in proxy mode,
  `_astro` assets get `immutable`, sidecar 404 is the site 404, unreachable sidecar → 502, and
  `None` = the old behaviour exactly.

## Verified

```
cargo: 477 tests green (470 at branch + 7 proxy ITs) · clippy -D warnings clean · conventions clean
astro build: 628 ms · vitest: --passWithNoTests

live, through the proxy on :8280
  /              200, Astro SSR HTML, all 7 books linked, CSP + security stack stamped
  /api/health    200 from axum, never proxied
  /robots.txt    axum, unchanged
  /sitemap.xml   263 urls, axum, unchanged
env unset        the old client serves — rollback demonstrated
```

**CSP: verified unchanged, not assumed.** The Astro page rides the existing policy — its inline
theme script is the same script the policy already allows, and its hydration scripts are
external `/_astro/*.js` under `script-src 'self'`.

## The lesson

**A migration's first deliverable is its off switch.** Nothing above renders a lesson or moves
a feature; what it builds is the property that every later step can be abandoned for the cost
of unsetting one env var — and that property is pinned by a test (`unset_is_byte_identical_yesterday`)
rather than carried as an intention. Thirteen steps of porting sit on top of this one; the time
to prove the escape hatch is before anyone is standing on it.
