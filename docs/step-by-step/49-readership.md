# Step 49 — Readership

*(442 lessons, and no way to know whether anyone opened one of them.)*

## Why this is first

The product assessment written before this step listed nine things worth building. This is the
smallest of them and it goes first, because every other item on that list is a guess until it
exists.

"Which problems should we deepen?" — unanswerable. "Is the system-design book worth the 355,000
words in it, or is everyone reading Python?" — unanswerable. "Did the meta-tag work in step 50
bring anyone?" — unanswerable, and worse, *unfalsifiable*: without a before, there is no after.

The instinct is to reach for Plausible or Umami. That would mean a third-party script, a CSP
directive, a cookie decision and a second service to run. None of it is necessary, because
**every lesson view already flows through one endpoint we own.**

## Half of it already existed

`catalog.lesson` has carried the lesson path as a span field since step 45:

```rust
#[tracing::instrument(name = "catalog.lesson", skip(self), fields(path = %path.join("/")))]
pub async fn lesson(&self, path: &[String]) -> Result<LessonContent, ContentError>
```

So the instrumentation was already right, and deliberately so — `platform/telemetry.rs` records
the *matched* route on the `http` span (`/api/synapse/{*paths}`) rather than the concrete URI,
because a raw URI makes every lesson its own span name and turns tracing into a bill. Per-lesson
attribution belongs on a child span, which is exactly where it was.

What was missing is that a span is not a record. It goes to stdout, and stdout goes nowhere in
particular. This step gives it somewhere to land and a way to read it back.

## Where it lives, and why not in `catalog`

`insights` is a new context. That deserves defending, because a new bounded context for one
table looks like ceremony.

`catalog` is a pure content-serving context whose single output port is a filesystem. Giving it
a Postgres port would make it dual-store for a concern that is not content — and the concern
genuinely is not content: readership is about the reader. It is not `submission` either, which
is about the reader's *code* rather than their reading.

So it follows the precedent step 18 set when the blog became a deliberate twin of the catalog
rather than a reuse of it: contexts own their vocabulary. It is flat and thin per ADR-S007, and
it has **no `domain/` at all** — a view is a timestamp and a path, nothing here has behaviour
worth modelling, and a domain layer would be the ceremony the new context is accused of.

## What the table cannot answer

```sql
create table lesson_view (
    id          bigserial   primary key,
    lesson_path text        not null,
    viewed_at   timestamptz not null default now(),
    authed      boolean     not null
);
```

No user id. No session. No IP. No referrer. This is a deliberate ceiling, not an oversight: the
only questions the table can answer are "which lessons get opened" and "which never do", and it
is physically incapable of answering "who read this" because it never recorded it. The route test
asserts the absence — `body[0].get("userId").is_none()` — so the property is pinned rather than
merely intended.

`authed` is the one bit of attribution, and it is honest about being approximate: **it counts
requests that PRESENTED a bearer token, not ones that verified.** Verifying would put a JWKS
check on the read path of every page view, which is a real cost for a bit that is only ever read
in aggregate. Documented at the port, at the call site, and in the migration.

## Fire and forget, decided at the call site

The port returns a `Result`. The catalog route throws it away:

```rust
if let Err(error) = state.views.record(lesson_path, authed).await {
    tracing::warn!(lesson_path, %error, "readership not recorded");
}
```

That split is the point. A store that is down must never cost a reader their lesson — but that is
a *policy*, and policy belongs at the call site, not baked into a port that a future caller might
legitimately want to fail loudly. Recording also happens only after the lesson resolves: a 404
is not a read.

The catalog router is generic over the store port rather than holding the Postgres adapter, so
`catalog/http` depends on `insights`'s contract and never on its infrastructure.

## The gate moved

`require_admin` lived privately inside `submission/http/admin.rs` since step 21. The readership
read is the second caller, so it moved to `platform/admin_gate.rs` with the invariant stated
once: **ADMIN is config (`ADMIN_USERS`), never a token claim, re-checked on every call.**

Two things surfaced in the move. Its audit line was hardcoded to `"allowlist call"`, which would
have been quietly wrong the moment a second admin route existed — it now takes the call name.
And clippy immediately fired `implicit_hasher` on the extracted free function, a lint the old
shape had hidden because the set was reached through `&self.admin_users` rather than passed.

## What this deliberately does not do

**No client UI.** There is no `/admin` panel section for this yet. The read is an authenticated
JSON endpoint, which is enough to answer the questions that unblock steps 50–57, and a chart
nobody has data for is furniture.

**No per-user analytics, ever.** Not deferred — excluded. The schema has nowhere to put it.

**No recording of index, blog or problem-page views.** Lessons only. The question that blocks the
backlog is "which lessons", and widening the surface before anything has been learned from the
narrow one is how a measurement feature becomes a tracking feature.

**No retention or rollup.** The table grows unbounded, one row per view. At homelab traffic that
is years away from mattering, and picking a retention window before seeing a week of real volume
would be inventing a number.

## Verified

Route ITs drive a fake through the REAL router (the `admin_allowlist_it.rs` pattern — implement
the port for `&'static Fake`): anonymous → 401, verified non-admin → 403 with the oracle's exact
copy, admin → counts most-read first with `lastViewed` as an ISO-8601 string. Both rejections
assert the store was **never reached**, so the gate provably runs before the read. Limit handling
is pinned across five calls: absent → 50, over → 500, zero and negative → 1.

The gated Postgres IT ran against the real database on :5532 — the dedicated `synapse_rs`, never
the oracle's Liquibase-managed `synapse`:

```
readership_counts_and_orders_by_views ... ok
readership_limit_is_honoured ... ok
test result: ok. 7 passed
```

It proves the aggregate, not just the insert: four views of one path and one of another come back
as two rows, most-read first, with `authed_views` counting only the bearer-carrying request. Both
tests take their own namespace under `it-rs` and clean only that, per the step-45 lesson about
shared prefixes under parallel test execution.

409 rust (+6) + 74 vitest. Conventions, fmt and clippy clean. Insomnia grew the request.

## The flake this step shipped, and the fix

The first push of this step went green locally and **failed in CI**, which is the correct way
round and still embarrassing, because the bug is the step-45 lesson repeating one level down.

The two Postgres tests take namespaces `it-rs-views` and `it-rs-views-limit`. The cleanup was
boundary-aware — `delete … where lesson_path like '{namespace}/%'` — but the *assertion filter*
was not:

```rust
top.iter().filter(|c| c.lesson_path.starts_with(&ns))   // ns = "it-rs-views"
```

`it-rs-views-limit` starts with `it-rs-views`. Under CI's default parallelism the sibling test's
three rows landed inside the first test's filter, and the "one row per distinct path" assertion
saw 5 where it expected 2. Locally the two happened to interleave the other way, so it passed —
which is what a race is.

Measured against the real database afterwards, which is what turned a plausible story into a
confirmed one:

```
like 'it-rs-views%'   → 5 distinct paths   ← the old filter, and the failing assertion
like 'it-rs-views/%'  → 2 distinct paths   ← the fix
```

The filter now matches on the namespace boundary, like the cleanup always did. Renaming one of
the namespaces would also have made the symptom go away, and would have left the next
prefix-shaped pair to find it again.

`step-49` was re-tagged onto the fixed commit rather than left marking a red tree — the build
book's invariant is that every tag compiles and its tests pass, and a tag whose CI never went
green has no history worth preserving.

## The lesson

**Instrumentation is not measurement.** The lesson path had been recorded on a span for four
steps, and the honest answer to "which lessons do people read" was still "no idea" — because a
span is a thing you can watch, not a thing you can ask. The gap between those two verbs was the
entire feature, and it cost one table and one route.

And a second, cheaper one: **the cleanup and the assertion have to agree about what a namespace
is.** Step 45 fixed shared-prefix bleed in the *deletes* and stopped there. Every place that
partitions test data by a string prefix is the same bug waiting — including the ones that only
read.
