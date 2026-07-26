# RS002 — Derivative study material never reaches the served catalog

**Status:** accepted · 2026-07-19 · **amended 2026-07-27** (see *Amendment* below)

## Context

The content tree carries two books that must not be published.

`local-only/system-design-swiftly` is 66 lessons and roughly 310,000 words — about a quarter of
all prose in the corpus. Its own `book.json` states what it is: *"adapted from Hello Interview's
'System Design in a Hurry'"*, with lessons that *"carry their original video (a public YouTube
walkthrough or a premium screencast)"*. It exists because it is genuinely useful to study from.
`local-only/01-sql` sits beside it, a further 50 lessons.

Keeping adapted material for private study is ordinary and defensible. Serving it is not, and
serving it from a public site with a sitemap advertising it to crawlers is a different thing
again.

Until step 54 the separation rested entirely on a `.gitignore` rule in the **content**
repository. That is the wrong repo, the wrong layer, and the wrong moment:

- **Wrong repo.** The rule lives in `synapse-content`; the code that decides what to serve lives
  here. Neither knows about the other.
- **Wrong layer.** It governs what is *committed*, not what is *served*. The walker indexed those
  books happily. They appeared in `/api/synapse/index`, and — once step 50 landed — in
  `/sitemap.xml`, and once step 49 landed, in `lesson_view`. Every developer instance served them
  at real URLs.
- **Wrong moment.** It applies at push time. The only thing standing between those lessons and
  production was that nobody had run the wrong git command yet.

Three ordinary actions defeated it silently: `git add -f`; a blanket `git add -A` (this project
has that scar — the step-42 note records a stray gitlink reaching public `main` exactly that
way); or any restructure that moved a book out of that folder. None looks like a mistake while
you are doing it, and the failure only becomes visible once a crawler has the URLs.

## Decision

**Material that must not be published is excluded by the walker, not by git.**

`local-only` joins `RESERVED_AUX_DIRS` in `catalog/domain/walker.rs`, alongside `examples` and
`c4`. The check runs order-prefix-stripped, so `01-local-only` is excluded too — an ordering
prefix must not quietly republish 66 lessons.

The `_`-prefix rule the walker already applies is a second, independent route to the same
outcome: renaming the directory to `_local-only` would also make it unservable. Both are kept
deliberately. One is a name anyone might change; the other is a decision recorded in code with a
test attached.

Two tests pin the behaviour: a well-formed book inside `local-only/` yields no catalog entry and
no reachable lesson path, and the order-prefix variant is excluded as well.

If this material is ever wanted publicly, the answer is to **rewrite it from primary sources**,
not to relax this rule. Adapting someone else's course was the wrong shape to begin with; the
right fix is different content, not different plumbing.

## Consequences

- The content repository can hold study material without that being a publishing risk. The
  `.gitignore` rule stays as defence in depth, but it is no longer load-bearing.
- Local development now matches production for this content: 7 books, not 9. Before this, every
  developer instance served two books production did not, which made "what does the catalog
  contain" a question with two different answers.
- `RESERVED_AUX_DIRS` now mixes two kinds of exclusion — aux dirs a book carries (`examples`,
  `c4`) and content that must not ship (`local-only`). That is a small semantic smudge, accepted
  because a second mechanism would be more machinery than the problem justifies. The constant's
  doc comment says which is which.
- Anything added to that list is invisible to the catalog with no other signal. A directory that
  silently produces no books is confusing if you have forgotten the rule, which is why the
  comment in `walker.rs` is long and points here.

---

## Amendment — 2026-07-27

Two changes, made together because the second is only safe given the first.

**The directory is now `local-only-content/`.** Both spellings stay in `RESERVED_AUX_DIRS`. A
checkout that has not caught up with the rename must not start serving 66 lessons because of it,
and one extra array entry is not a trade worth deliberating.

**A `render-local-only` cargo feature renders it — locally.** Studying from material you cannot
open is a poor arrangement, and the dev loop is the one place where reading it is exactly right.

The gate is a cargo feature and NOT an env var, which is the whole point. An env var would turn
this decision's guarantee — unservable *by construction*, in the repository that does the serving —
back into a deployment-config promise, which is the shape this ADR was written to reject. The
production binary does not contain the branch at all:

- `dev-tools/dev` is the only thing that enables it.
- `check-conventions.sh` fails if `Dockerfile` or `start.sh` ever does (verified in both
  directions, not merely written down).
- `dev-tools/e2e` stays on the prod-shaped binary, without the feature.

One consequence needed closing. Under the feature the study trees ARE in the catalog, so the
editor's resolver finds them — and committing one would *publish* a gitignored file, which is
precisely the outcome this decision prevents. `FsLessonSource` therefore refuses them
unconditionally, not behind the same feature: a guard that exists only in the build that needs it
is one refactor away from not existing.
