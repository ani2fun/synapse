# Step 61 — The bearer skeleton, stated once

*(A security invariant maintained by convention in four places becomes one function with
four policies.)*

## The ask

Item 5 of the deepening loop. The rule "an absent bearer is anonymous; a PRESENT bearer must
verify — bad tokens 401, never silently anonymous" was re-implemented as the same
`bearer → authenticate → to_auth_error` skeleton in four places: the execution run gate,
submission's `caller_user`, identity's own `get_me`/`delete_me`, and
`platform::admin_gate::require_admin`. Each copy was correct today; the risk was locality —
a security rule you fix in one place and quietly not in three others. `admin_gate` (step 49)
had already proven the extraction pattern for the admin flavour; this step extracts the
layer beneath it.

## The shape

`identity::http::optional_user(identity, headers) -> Result<Option<AuthenticatedUser>, Reject>`
— identity already owned `bearer` and `to_auth_error`, and every gate already imported from
it (including `platform::admin_gate`), so the seam introduces no new dependency direction.
The doc comment carries the invariant sentence, moved from submission's `caller_user`.

What deliberately did NOT move — each caller keeps its own **policy** and **copy**:

- execution `run_code`: anonymous allowed → metered per IP; verified → per subject. Now one
  line: `optional_user(…).await?.map(|user| user.id.0)`.
- submission: `caller_user` stays as the local one-line delegation (call sites read the
  same); `needs_token(verb)` and its per-verb 401 copy stay local — anonymous means `[]`
  for list, 401 for delete/erase, allowed for submit.
- identity `get_me`/`delete_me`: anonymous → the local `missing_token()` copy.
- `admin_gate::require_admin`: resolves via the skeleton, keeps its own 401
  ("Admin calls require a signed-in admin") and the 403 "Admin only" verdict.
- catalog `record_view` is **unchanged and unshared on purpose**: it is presence-only
  (`bearer().is_some()`, no verify) so a page view never costs a JWKS round-trip — a
  different contract, documented at the site, and forcing it through `optional_user` would
  change semantics.

`grep '\.authenticate(' server/src` outside the application layer now hits exactly one
line — inside `optional_user`.

## Verified

Conventions · fmt · clippy pedantic (clean) · `cargo test --workspace` 458 · vitest 83. The
proof of behaviour-identity is that the gate ITs — junk-bearer 401 per context, anonymous
list → `[]`, anonymous delete → 401, admin 401/403 copies verbatim, JWKS-down → 503 — all
pass without touching a single assertion; they were written against the four copies and now
pin the one skeleton.
