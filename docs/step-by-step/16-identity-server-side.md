# Step 16 — Identity, server side: the JWKS verifier and the user seams

*(oracle: synapse step 17's server half with step 36's lowercase canonicalisation folded in as
final design — `JwksTokenVerifier`, `IdentityService`/`AuthError`, `GET /api/auth/config`,
`GET /api/me`, the optional-bearer seams into submissions; `JwksTokenVerifierSpec` +
`IdentityRoutesSpec` ported. Account deletion and the admin flag stage later, as they did.)*

## The verifier (`identity/infrastructure/jwks.rs`)

The pipeline verbatim: RS256 against the realm's JWKS (lazy first fetch, 5-minute cache, ONE
forced refresh on an unknown `kid` — key rotation, not an error), exact `iss`, `exp` with 60 s
leeway, required `{sub, exp, iss}` — then the **manual Keycloak audience quirk**: public SPA
tokens carry `aud:["account"]` and name the client only in `azp`, so the rule is
`aud ∋ clientId OR azp == clientId` with library `aud` checking OFF. Usernames leave the
verifier **canonical lowercase** (the step-36 audit fix, applied once, so the admin gate and the
submit allowlist later compare apples to apples). The two-way degrade: JWKS unreachable →
`VerifierUnavailable` (503 — IdP-down is OUR problem); everything else → `InvalidToken` (401).

## The surface and the seams

`GET /api/auth/config` splits the issuer into `{url, realm, clientId}` — exactly
`new Keycloak({…})`; a non-Keycloak-shaped issuer is a loud 500 with the operator hint.
`GET /api/me` echoes the verified caller (`admin: false` until the admin step — UX-only, the
server re-checks anyway). Submissions grow their identity seams, the rule stated once and
enforced everywhere: **absent bearer = anonymous; a PRESENT bearer must verify — bad tokens
401, never silently anonymous.** Submit stores the verified `sub`; the list becomes PRIVATE
(anonymous → `[]`, store untouched); owner-only `DELETE /{id}` (403 `NotYours`) and the
erase-all verb land with their port + Postgres methods.

## Tests

Nine identity ITs against a LOCAL JWKS stub with in-test-minted tokens (a committed test-only
RSA key): the azp branch + lowercase pin, the aud branch + sub fallback, expired/wrong-issuer/
foreign-audience/garbage/missing → 401, unreachable realm → **503 never 401**, the auth-config
split and its loud 500, the private-list and owner-verb seams, and
bad-bearer-never-silently-anonymous. Suite: 159 Rust + 40 vitest.

## Verified against the REAL realm

Compose Keycloak 26 (realm `synapse`, dev user `tester`): a real direct-grant token through the
live Rust server → `GET /api/me` returns the real `sub` + `tester` lowercase; submit-as-tester
stores the user; tester's list shows exactly their row while the anonymous list is `[]`; owner
delete returns `{"deleted":1}`; anonymous delete 401s. Insomnia grew me/auth-config/delete/erase.

Next: the client half — keycloak-js PKCE boot, `AuthStore`, the account chip, bearer injection,
and the Edit/Submit gates.
