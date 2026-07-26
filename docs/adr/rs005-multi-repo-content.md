# RS005 — The library is assembled from many content repositories

**Status:** accepted · 2026-07-27

## Context

Every book was served from one checkout. `SYNAPSE_ROOT` pointed at a git-sync'd clone of
`ani2fun/synapse-content` and `CONTENT_REPO` named the one repository the in-app editor opened
pull requests against. Prose, media, problems and proposals all lived in that single tree.

That is the wrong shape for how the content is growing. A Java guide, a Python guide and a
low-level-design guide have their own cadence, their own reviewers and — the point — their own
potential *outside* maintainers. Someone who should be improving the Java book has no business
needing write access to the system-design book, and today they cannot have one without the other.

The constraint that shapes everything below: **the reader must not be able to tell.** Readership
and progress rows are keyed on the joined lesson path, intra-book links are written app-absolute
into the prose, and the sitemap is indexed. A split that moved URLs would orphan all of it.

## Decision

**The library is the merge of N content sources, and which sources exist is a row, not a redeploy.**

### Two source shapes, told apart by the checkout itself

| Root holds | Shape | Walked as |
| --- | --- | --- |
| no `book.json` | **collection** | today's behaviour: categories and books by directory nesting |
| a `book.json` | **book** | the whole repository is one book, chapters at the root |

No manifest, no configuration: a repository that carries a root `book.json` *is* a book. That is
also why root-level markdown became a lesson — `index.md` at the root of a book repository is that
book's opening page, where at a collection root it is nothing.

### The registration row owns placement; `book.json` owns identity

The admin row carries the repository, branch, **grouping** and **order**. `book.json` carries
title, description, tags and — critically — **`slug`**.

The split is not arbitrary. What a book *is* travels with the book, so a maintainer controls it
without asking. Where it *sits* is the platform's business, so a book can move without a content
push. But the slug is the URL, so it lives with the content: deriving it from the repository name
would let a rename silently move every lesson, and putting it in an admin field would let a typo
do the same. A book source with no slug therefore falls back to the source id and says so loudly,
and `validate-book` refuses it outright.

### First source wins, and the primary is always first

Cross-source conflicts are **warnings, not errors**. Refusing to serve the whole library because
one satellite clashes is worse than serving the winner and saying so.

This is what makes a migration safe. While a book exists in both the monorepo and its new
repository, the monorepo's copy serves — so the satellite can be registered, fetched and verified
against the real URLs *before* anything is deleted. There is no atomic flip to get right and no
window where the catalog is empty.

The subtle part is that a skipped book must forfeit its `lesson_files` entry too. Keeping the
catalog node and overwriting the file map would leave the winning book serving the loser's paths.

### Satellites are fetched over REST, not git-sync

The primary keeps its git-sync sidecar. Satellites cannot have one: sidecars are declared in the
deploy manifest, and the whole point of a registry is that adding a repository is not a redeploy.
So the server fetches tarballs from the GitHub API — the same "no git binary, no working copy"
stance ADR-RS004 took for the authoring forge.

The cache mirrors git-sync's layout because its atomicity trick is the right one:
`<cache>/<id>/<sha>/` written in full, then a `current` symlink flipped onto it, then older
commits pruned. A reader either follows `current` to a complete checkout or to the previous one.

Unpacking is **untrusted-archive handling** and is treated as such: traversal, absolute paths and
links are refused in the unpacker itself, with size and entry caps, because by the time a caller
could validate a path the write has already happened.

A failing source never takes the library down. It keeps serving its last good commit, the error
lands on its row for an admin to read, and a source that has never landed is simply an absent book.

### Edits route to the owning repository

A **new** proposal goes to whichever source owns the lesson now. A **revision** goes to the
repository recorded on its own row. Those diverge exactly when a book migrates mid-review — and
the open branch still lives where it was created, so re-routing by the page's current source would
commit into the wrong repository. `content_edit_request.repo` exists to make that followable, and
`branch` uniqueness became `(repo, branch)`: two repositories can carry the same branch name.

### One merged C4 workspace, gathered from many repositories

The `/c4` viewer stays ONE workspace with exactly one `specification {}`. Its build gathers `.c4`
files from the registered sources rather than from one checkout. Keeping it merged is what lets
every existing `<iframe src="/c4/view/…">` keep working untouched — view ids encode no repository.

## Consequences

- **URLs are unchanged**, which is the whole point. `/media` and the sitemap follow from that.
- The catalog stays **one globally cached tree**. Nothing here makes a response per-reader.
- `validate-book` becomes the gate a satellite maintainer runs, using the server's own walker so
  the gate and the site cannot disagree.
- **The migration precondition is exact**: a satellite's grouping and `book.json.slug` must
  reproduce the path the book had in the monorepo. Get it wrong and deletion day moves every URL.
- `.c4` in a satellite renders nothing until the gathering build is deployed, and it fails
  *silently* — a blank iframe. The split of a diagram-carrying book is gated on it.
- The spine repository does not become obsolete. It keeps the blog, the category declarations and
  the shared C4 specification, and sheds books over time.

## Alternatives considered

**A mount prefix in server config.** Rejected: relocating a book would become a redeploy, and the
registry exists precisely to avoid that.

**One `/c4` SPA per repository.** Rejected: it would mean rewriting every iframe, duplicating the
specification into every repository and keeping it in sync, and running N nginx deployments — for
no gain over gathering into the merged workspace.

**Per-user private repositories.** Deferred to its own decision. It forces the catalog to stop
being one globally cached tree — per-subject cache-control, ownership checks on lessons *and*
media, and a sitemap that knows what to omit. That is a data-leak surface, and not worth carrying
to get satellite repositories working.
