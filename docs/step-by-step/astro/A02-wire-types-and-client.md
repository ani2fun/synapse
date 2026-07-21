# A02 — Wire types and the typed client

*(a hand-typed interface against a wire contract the server already publishes is a guess with
syntax highlighting — A01 said so; this step is what removing the guess actually looks like.)*

> Branch chapter: the Astro migration runs on branch `astro`, numbered A01–A14, folded into the
> main ledger at merge. Main is at step-65 and keeps moving underneath.

## What this step is for

Three things, landing together because the second and third depend on the first existing:

1. **Generated wire types.** `server/src/lib.rs`'s `ApiDoc` — the same utoipa document
   `contract_it.rs` already diffs against the oracle spec — is rendered to `schema.gen.ts` by a
   small pipeline: a new binary prints it as JSON, `openapi-typescript` turns the JSON into
   TypeScript, a script wires the two together and CI proves the result never goes stale.
2. **The typed client.** `web/src/lib/api/client.ts` is a line-for-line port of
   `client/src/api/mod.rs` — the same 18 endpoints, the same bearer seam, the same error-message
   format — now speaking the generated types instead of the Rust wire structs.
3. **The first two pure-logic ports.** `client/src/router/page.rs` → `routes.ts` and the pure
   half of `client/src/seo.rs` → `seo.ts`, each with the same test coverage the Rust module had.
   Small on their own; what they prove is that "port a pure module, keep its tests" is a move
   this migration can repeat without re-deriving the approach each time.

## Why generated, not hand-written

A01's placeholder page guessed the index's wire shape from field presence
(`"categoryPath" in entry`) and silently rendered one book of seven, because the real shape is
kind-discriminated (`kind: "book" | "category"`) and the guess never read the tag. That bug was
cheap to fix locally, but the argument it leaves standing is not local: **any interface typed by
hand against a contract the server already renders is unchecked by construction.** It compiles
whether or not it agrees with the server, because nothing compares the two. The server has
published that contract as OpenAPI since step 06 (`contract_it.rs` proves the render against the
oracle spec on every `cargo test`); the missing piece was reading it as the spec rather than
retyping it as prose a second time, by hand, in a different language.

The pipeline is three small pieces:

- `server/src/bin/dump_openapi.rs` — prints `ApiDoc::openapi().to_pretty_json()` to stdout.
  Nothing else: no port to bind, no config to load, no Postgres to reach. Generating the
  document needs the `#[derive(OpenApi)]` macro output and nothing more.
- `dev-tools/gen-api-types.sh` — builds and runs that binary, pipes the JSON into
  `npx --yes openapi-typescript@latest` from inside `web/` (so it shares `web`'s npm cache), and
  writes `web/src/lib/api/schema.gen.ts` with a header naming the regenerate command. Running it
  twice back to back produces byte-identical output — checked, not assumed, before wiring the CI
  diff around that assumption.
- The `web` CI job runs the same script and `git diff --exit-code`s the one generated file. A
  schema that drifted from a hand edit, or a server change nobody regenerated for, fails the
  build instead of waiting for a reviewer's eye. The job had no Rust toolchain before this step
  (it only ever ran `npm`); `dtolnay/rust-toolchain` + `Swatinem/rust-cache` now install *before*
  the npm steps, not just before the step that happens to need them — so the ordering reads as a
  dependency, not an accident of where someone happened to paste a step. Budget raised 15 → 20
  minutes for the same reason: a cold `cargo build -p synapse-server` with no prior cache made
  the old figure tight.

**The generator found a bug `cargo test` could not see.** `BookDto.entries` and
`ChapterDto.entries` carry `schema(no_recursion)` — they are genuinely self-referential trees,
and the attribute stops utoipa's auto-discovery walk from recursing through them. That is exactly
what it is meant to do, and it also meant `BookEntryDto` and `ChapterDto` were never reachable
from anything in `ApiDoc`'s explicit schema list, so the rendered OpenAPI document has held a
dangling `$ref: '#/components/schemas/BookEntryDto'` for as long as the field existed. Nothing
caught it: `contract_it.rs` only checks that schemas *the oracle also names* are present and
shaped right, and the oracle's Scala spec does not name a `BookEntryDto` at all. `cargo test` was
green the whole time it was broken. `openapi-typescript` does not extend that courtesy — it tried
to resolve the reference, found nothing at that path, and the generated type would have been
unusable for exactly the recursive book-and-chapter tree `index.astro` needs to walk. The fix is
two lines (`BookEntryDto` and `ChapterDto` added to `ApiDoc`'s explicit schema list, with a
comment explaining why they were never auto-discovered); the finding is the chapter's real
argument. A stricter reader surfaces a gap sooner than a lenient one, even when the lenient one is
a test suite.

## The bearer seam

Identity does not exist in `web/` yet, so the client has to stay usable before it and unchanged
after it — the same requirement the Rust client already solved with a `thread_local` holding
`fn() -> Option<String>`, installed once and read by every request. `client.ts` mirrors it with a
module-level `let tokenProvider` plus `installTokenProvider(provider)`, and every one of the 18
endpoint functions routes its headers through the same `bearerHeaders()`. The point of the seam
is the same in both languages: `api`/`client.ts` never imports identity, so identity landing later
is a new caller of `installTokenProvider`, not a rewrite of every fetch.

The error format is ported with the same care. Rust's inline `decode` turns a non-2xx response
into `envelope.detail.map_or(envelope.error, |d| format!("{error}: {d}"))`, falling back to
`HTTP {status}` when the body was not the `ApiError` envelope at all (a stripped proxy body, a
502 from something that has never heard of the shape). `ApiFailure` in `client.ts` formats
identically, so error copy shown to a reader does not change mid-migration. One small, documented
departure: the Rust client hand-rolls `allowlist_revoke` separately from the shared `decode`,
because its 204 has no body and Rust has no single `void` every `Result<T, String>` can unify
around. TypeScript does — `decode<T>` treats `T = void` as a legal target for `undefined` — so the
204 case folds into the same one chokepoint instead of getting a bespoke twin.

## What this deliberately does not do

- **No per-call debug logging.** The Rust client logs one line per endpoint
  (`crate::log::debug`); nothing in `web/` reads logs like that yet, so there is nothing to wire
  it to. It can join when a caller needs it, not preemptively.
- **`openapi-typescript@latest`, not pinned.** CI runs the exact command a developer would, so a
  version bump that changes the output is caught as "drift" the same way a server change would
  be — just with a less specific root cause in the error. That is an acceptable trade for now;
  if `@latest` ever produces a diff nobody actually caused, pinning is the fix.
- **`schema.gen.ts`'s `paths`/`operations` interfaces go unused.** `openapi-typescript` emits an
  `openapi-fetch`-shaped typed client alongside the schema types — a heavier pattern than 18 hand
  -written functions mirroring the Rust client's own shape. Adopting it mid-port would be a
  second decision riding on this step's back; the DTOs under `components["schemas"]` are what
  this step needed.
- **`routes.ts` wires into nothing yet.** Astro's file-based routing owns URL → page today;
  `Page`'s job in the old client was interpreting arbitrary segments for a client-side router,
  which does not exist here. It is ported anyway, with its tests, because this step is explicitly
  about proving the "pure module in, same coverage out" move before later steps lean on it
  repeatedly — not about wiring it to a caller that does not exist yet.
- **`seo.rs`'s DOM half does not port.** `set_title`/`set_description` existed to patch a stale
  tab after a client-side SPA navigation that never re-fetched the document. Astro's
  `output: "server"` re-renders the whole head, as props, on every navigation (A01's
  `layouts/Base.astro`) — there is no stale moment for a client-side patch to fix. Only the
  *format* (`Book · Lesson — Synapse`) survives, because it still has to agree with wherever the
  server computes a title.

## Gates

- `check-conventions.sh` gained one exemption: `*.gen.ts` is excluded from the 800-line file cap
  (both under `client/` and `web/`, for whichever side ever grows one). `schema.gen.ts` is 1,418
  lines of machine output; the cap's reasoning — a file that size is doing too much and should
  split along a seam — has no seam to offer a generated schema. The same exemption the cap
  already gives `dist/`/`node_modules/`, extended to a generated *file* rather than a whole
  directory.
- `cargo fmt --all --check` · `cargo clippy --workspace --all-targets --all-features -- -D
  warnings` (pedantic, `dump_openapi.rs` included) · `cargo test --workspace` — all clean.
- `(cd web && npm test && npm run build)` — vitest now runs for real (`--passWithNoTests`
  retired now that `routes.test.ts`/`seo.test.ts` exist); `astro build` unchanged in shape from
  A01 (no running API needed — `output: "server"` fetches at request time).
- `(cd client && npm test)` — the old client's 83 vitest tests, untouched by this step, still
  green.

## Verified

```
cargo:  477 tests green (unchanged from A01's branch point — this step's Rust surface is one
        binary + a two-line schema-completeness fix, neither test-driven)
        clippy -D warnings clean · cargo fmt clean · conventions clean (incl. the new *.gen.ts
        exemption)

web vitest: 4 tests, 2 files, 119ms
  routes.test.ts
    parses the app map .............................................. ok
    urls round trip .................................................. ok
  seo.test.ts
    the book leads the lesson title .................................. ok
    an unknown book still produces a usable title ..................... ok
astro build: server built in 660ms

client vitest (must stay green, untouched): 83 tests, 3 files

schema.gen.ts: regenerated twice back to back — byte-identical (dev-tools/gen-api-types.sh is
deterministic given the same ApiDoc)

live, through dev-tools/dev (axum :8280 + astro dev :5373, real Postgres, real
synapse-content checkout)
  GET /api/synapse/index    200, decoded through fetchIndex() + the generated SynapseIndex type
  /                          all 7 books rendered (Synapse Features, System Design from First
                             Principles, Low Level Design, DSA, Python, Java, Synapse App From
                             Scratch) — including both programming-languages/{python,java}
                             category-nested books, screenshot-verified in dark mode
```

## The lesson

**The dangling `$ref` is the argument, not a footnote to it.** Generating types instead of
hand-writing them is easy to justify as "less typing" and leave there — true, but not the point.
The point is that a generator has to actually resolve every reference to produce anything at all,
where a human transcribing a shape by hand only has to *believe* they got it right, and a test
suite only catches what someone thought to assert. `BookEntryDto` sat unreachable in the
OpenAPI document through every prior step because nothing before this one ever asked
`openapi-typescript`'s question — "does this reference resolve?" — of that specific document.
The fix cost two lines. Finding out it was needed cost nothing but pointing a stricter reader at
work that already existed.
