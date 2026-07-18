# Step 44 — Automatic releases

*(the release stops being a button; plus the phone-header bug that had been quietly setting the
header's height, and the sidebar work steps 42–43 shipped without a chapter.)*

## The release

`build-push-promote.yml` was `workflow_dispatch:` only — a deliberate hold from 2026-07-17 while
pre-cutover fixes landed, kept in place through the cutover itself. That hold is over, so the
button is gone: **every push to `main` that clears the gates now ships**, build → ghcr → patch
the image tag in `ani2fun/infra` → ArgoCD rolls it out.

The interesting part is where it lives. The release is a **job inside `ci.yml`**, not a workflow
of its own, and that is the whole design:

- **It has to say `needs:`.** The gates and the release must be one dependency graph or they
  race. Two workflows both triggered by `push` start together, so a release would be underway
  while clippy is still running — gated in appearance only. As a job it simply cannot start
  until `conventions`, `build-test` and `client-build` are green.
- **`workflow_run` would have fixed the ordering and broken the SHA.** It is the usual answer to
  "run after that other workflow", but in a `workflow_run` context `github.sha` is the default
  branch's tip rather than the commit that triggered the run. The engine tags the image and
  writes the promoted tag from `github.sha`, so under two quick pushes the image and the
  promotion could name different commits. A plain `push` job has the right SHA by construction.
- **Tags are excluded.** `ci.yml` also fires on `step-*` tags, and those point at commits `main`
  has already shipped; releasing again would rebuild an identical tree under a second SHA. The
  guard is `github.ref == 'refs/heads/main'`, not merely "is a push".

Two details that are easy to get wrong:

`concurrency: cancel-in-progress: false`. Cancelling a release mid-flight can push the image and
die before patching infra — the cluster stays on the previous tag with nothing to indicate that
anything was missed. A queued release is fine; a half-applied one is not.

The `docker` smoke job now **steps aside on main** (`if: github.event_name != 'push' || github.ref
!= 'refs/heads/main'`). It exists to prove the Dockerfile still builds; on main the release job
does that same build for real, and running both would build the image twice per push. The two are
mutually exclusive by `if` and share no `needs` edge, so neither can skip the other.

The manual caller is deleted rather than kept as an escape hatch. Two paths that both push
`:latest` and patch the same kustomization can interleave, and GitHub's "re-run jobs" on the ci
run already covers the one case a dispatch was useful for.

Rollback is unchanged and still `kubectl`-free: revert the promote commit in `ani2fun/infra`.

## The phone header

`.header__search` is `width: min(24rem, 100%)` inside a flex row. On a phone the row shrank it to
55px while its content still needed 99px — and because a flex child's default `min-width` is
`auto`, the label refused to shrink with its box and simply escaped it, colliding with the nav
beside it and wrapping the bar to three lines.

That is where the header's **95px height on mobile** came from, which is worth naming: it is the
number that defeated the first attempt at the nav drawer fix in step 43, when the drawer was
offset by the desktop header's 65px and still ended up underneath. A layout bug in one component
had been quietly setting a constant another component was written against.

The root fix is `min-width: 0` plus ellipsis on the label, so it truncates inside its box at any
width instead of escaping. Below 640px the trigger then collapses properly: label and `⌘K` hint
stand down (there is no ⌘ on a phone to hint at) and it becomes a square icon button. The search
glyph is new — the button had no icon, so it had nothing to be once the words were gone.

Header height on a 375px viewport: **95px → 56px**, one row, no overflow. Desktop is untouched at
65px with the full box, and gains the magnifier.

## The sidebar work, recorded late

Steps 42–43 shipped two changes with no chapter. They belong in the book:

**Single-lesson folders flatten.** A chapter whose only child is a lesson of the **same slug** is
pure nesting — `pattern-01/pattern-01.md` is a directory because the lesson has sidecars
(`.editorial.md`), not because there is a chapter to browse. Those render as one lesson row, so
DSA's problems are one click instead of opening a folder to find its own namesake. 36 of the
catalog's 61 chapters qualify.

Same-slug is the discriminator, deliberately, not "has one lesson":
`low-level-design/basics` holds a single lesson named `intro`, and there the folder name *is*
saying something the lesson title doesn't, so it keeps its row. A folder named after its only
child is the one carrying no information.

**Books read as titles.** In the ← Learn browse view they were 14px links at `foreground/0.75` —
the same weight as the lesson rows inside a book, so a shelf of nine read as one flat run. They
are bold now, at the same `--foreground` as the category headers above them. They were briefly
teal, which was wrong for a reason worth keeping: teal is this reader's "you are here" signal —
the active lesson's border, the current chapter, the Run button — and spending it on all nine
books made the shelf read as nine selections while leaving the genuinely active book nothing to
say with.

## The lesson

**A gate that runs beside the thing it gates is not a gate.** The release workflow and CI were
both correct in isolation and both triggered by the same push, which reads as "CI passes, then we
ship" and is nothing of the sort. Ordering between workflows is not a thing GitHub gives you for
free; ordering between jobs is. When the sequencing matters, they belong in one graph.
