# Step 48 — Docs-only commits stop shipping

*(a two-line gate that would have been a no-op, and the glob idiom that reads like set subtraction but isn't.)*

## Why now

Step 44 made releases automatic: every push to `main` that clears the gates builds an image,
pushes it to ghcr, patches `ani2fun/infra` and lets ArgoCD roll production. No dispatch, no
button. That was the right call and it has been proven end to end.

What it never had is a paths filter. A commit that only edits a chapter takes the same route as
one that rewrites the executor — full build, full push, full rollout. Harmless once. But the
next stretch of work (the backlog in `product-assessment.md`, sequenced as steps 49–57) contains
a pure-docs step and a pure-ops step, and several of the code steps end with a separate chapter
commit. Left alone, that is four or five production rollouts that ship a byte-identical tree.

Worse than the waste: the deploy history stops meaning anything. "What was released on the 19th"
should be answerable, and a list where half the entries changed no shipped file does not answer it.

## The obvious fix is a no-op

The idiomatic answer is `dorny/paths-filter` with a subtraction filter:

```yaml
code:
  - '**'
  - '!docs/**'
  - '!**/*.md'
```

That reads as "everything, minus the docs". It is not what it does. paths-filter matches with
picomatch, and **picomatch ORs its patterns** — it does not treat a leading `!` as a subtraction
from the set built by the patterns before it. Each pattern is its own predicate:

```
picomatch(['**', '!docs/**', '!**/*.md'])

  docs/step-by-step/48-foo.md   →  true    ← '**' matches it
  README.md                     →  true    ← '**' matches it
  server/src/lib.rs             →  true
```

Every file matches. `code` is always `true`, the gate never fires, and nothing about the release
behaviour changes. It would have looked correct in review, passed CI, and quietly done nothing —
which is the worst outcome available, because the next person to wonder why docs commits still
deploy would have found a filter that appears to already handle it.

Checked against picomatch directly before writing a line of the workflow, under both plausible
evaluation strategies (one matcher compiled over the whole list, and any-of over individually
compiled matchers). Same answer both ways.

## The inverse fails in the dangerous direction

The natural repair is to invert it — enumerate the code paths instead:

```yaml
code:
  - 'server/**'
  - 'client/**'
  - 'migrations/**'
  - …
```

This works, and it is a trap of a different shape. An allow-list fails **closed**: the day
someone adds a top-level directory and forgets this list, a real code change stops shipping, main
and production silently disagree, and there is no signal anywhere. Subtraction fails **open** —
forget to exclude something and you merely deploy when you didn't need to, which costs five
minutes and announces itself in the run log.

For a gate that is an optimisation rather than a safety control, open is the only acceptable
direction.

## What shipped

A `changes` job computing the answer in plain git, which is also how
`dev-tools/check-conventions.sh` earns its keep — that gate is deliberately grep/find/wc so it
can run before any toolchain exists, and this question is genuinely one `git diff | grep -v`:

```sh
changed=$(git diff --name-only "$BEFORE" "$GITHUB_SHA")
rest=$(printf '%s\n' "$changed" \
  | grep -vE '^docs/|\.md$|^dev-tools/synapse-rs\.insomnia\.json$' || true)
[ -n "$rest" ] && code=true || code=false
```

`release` gains `needs: changes` and one clause:

```yaml
if: >-
  github.event_name == 'push' && github.ref == 'refs/heads/main'
  && needs.changes.outputs.code != 'false'
```

`!= 'false'`, not `== 'true'`. An empty or unexpected output still ships. The only thing that
stands the release down is the filter affirmatively saying "docs only". A base commit that cannot
be resolved — a new branch, a force push, a shallow fetch — takes the same fail-open path and
says so in the log.

`fetch-depth: 0` on the checkout, because the default shallow clone cannot diff against
`github.event.before`.

## What this deliberately does not do

**It does not gate any test job.** `conventions`, `supply-chain`, `build-test`, `gojudge-it`,
`client-build` and `docker` are untouched, and a docs-only commit still clears every one of them.
Only the shipping is skipped. A chapter that breaks a markdown link the conventions gate cares
about should still fail — the gate is about what reaches the cluster, not about what gets checked.

**It is not a workflow-level `paths-ignore`.** That would skip the whole run, tests included,
which is gating in appearance only — the same reasoning that put the release in `ci.yml` as a job
rather than in a workflow of its own back in step 44.

**It does not touch the `docker` smoke job.** On a docs-only push to main, `release` now stands
down and `docker` is still excluded by its own `if` — so that push builds no image at all. That
is the intent, not an oversight: there is no tree change to smoke-test.

## Verified

The gate logic run against real history, which is the only test that matters here:

```
d3e99ba  docs(step-31): fold the diagram fix into its home chapter   1 file    code=false  ← skips
bbeba11  refactor(reader): remove focus mode                         4 files   code=true
f9ddd13  feat(reader): the problem page remembers your tab           16 files  code=true
663cdcd  fix(reader): the mobile layout                              6 files   code=true
```

The one docs commit in recent history is the one that stands the release down. Workflow parses;
`changes` has no `needs` so it starts immediately and never delays the graph; no test job gained
a dependency on it.

This step's own commit touches `.github/workflows/ci.yml`, so it ships — correctly. The gate
cannot skip the change that creates it.

403 rust + 74 vitest (unchanged — no Rust or TypeScript in this step). Conventions and `cargo fmt
--all --check` clean; clippy and the suites not re-run, honestly, because nothing they cover moved.

## The lesson

**A gate you have not watched fire is a gate you have not written.** The subtraction filter was
the version I would have shipped on any other day: it is the documented-looking idiom, it reads
correctly in a diff, and it produces a green run. The only thing separating it from working code
was ten seconds spent asking picomatch what it actually does — and the failure mode it would have
left behind is invisible by construction, because a filter that never fires looks exactly like a
filter with nothing to do.
