# RS011 — Private books: a registered repository served to its reader list

**Status:** accepted · 2026-09-15 · **extends [RS005](rs005-multi-repo-content.md)** (the source
registry)

## Context

Every book the platform served was public by construction. The catalog took no identity: a lesson
was returned to whoever asked for its URL, the index listed every book, search ranked every source's
text, and the sitemap announced it all. That was the right shape for a library of published guides,
and it left no room for the one kind of content a reader produces themselves — study notes, the
rewrites of chapters they have worked through — which is theirs, and not for the world.

The repository side already half-worked. A satellite is fetched over the GitHub API with the
server's token (RS005), so a PRIVATE repository lands exactly like a public one provided the token
can read it. What was missing was the other half: who may read what landed.

Two facts shaped the design.

- **The session lives in the browser.** Identity is a Keycloak token held by `keycloak-js`; the
  page tier renders a lesson from an anonymous server-side fetch and has no cookie to forward. So
  the server can never prove, at SSR, who is asking.
- **The read path is hot and the bit is rare.** `record_view` already declined to verify a bearer
  per page view because a JWKS round trip on every read is a real cost. A gate that verified every
  request would tax every public lesson to protect a handful of private ones.

## Decision

**A registration carries a `visibility` and, when private, a reader list.** `content_source` gains
`visibility in ('public', 'private')`; `content_source_reader` holds `(source_id, username, note,
granted_at)`, the username canonical — trimmed and lowercased through `identity::Username::parse`,
the same rule as the submit and content-editor allowlists — so a grant is stored under the
spelling the reader's token will carry. The list belongs to the REGISTRATION, never to the
repository: a `book.json` is authored inside the repository and cannot be trusted to declare
itself public. A local satellite (`SYNAPSE_LOCAL_SOURCES`) declares `private` and `readers` the
same way, which is how the e2e suite proves the gate without a registry row or the network.

**The gate lives in the application service, keyed by source.** The merge already decides which
source serves a book (first source wins a slug); `WalkResult.book_sources` now exposes that map,
and `CatalogService` consults an `Audiences` cache — republished by the sync loop beside the
placements — to answer for a book. Every read path asks:

| Path | Anonymous | Signed in, not listed | Listed |
|---|---|---|---|
| `GET /api/synapse/{lesson}` | 401 `Sign in to read this book` | 403 `This book is private` | 200 |
| `GET /api/synapse/index` | the book is absent | absent | present, marked `private: true` |
| `GET /api/synapse/search` | no hits from it | none | hits |
| `/sitemap.xml` | absent | absent | absent — a crawler is nobody's reader |

The refusal is TYPED — `ContentError::Forbidden { book, anonymous }` — because the edge branches on
it (RS001): anonymous means "sign in", named means "ask the admin", and the two are different
actions for the reader. Both name the book. A 404 would hide nothing (the URL names the slug) and
would send the reader to the wrong fix.

**Verification happens only when it can matter.** A lesson route first asks `restricts(path)`;
a public lesson is served without touching the verifier, even when a bearer was sent. Only a
private lesson verifies — and then a bearer that fails is 401, never a silent fallback to
anonymous. The index and search verify when a bearer is PRESENT (present means "may be a reader")
and skip it otherwise, so the anonymous public tree costs exactly what it cost before.

**Audiences are not version-gated.** The catalog snapshot is keyed by content version and rebuilt
only when content moves. A reader granted at ten o'clock must be admitted at ten, so the audience
lookup is per request against the live cache, and a grant or revoke wakes the sync loop the way
"Sync now" does. The snapshot itself is unchanged: the search index holds every source's text and
filters by source at query time, before the candidate cut, so a private hit never displaces a
public one and never leaves the process.

**A private page renders in the browser.** The page tier ships the reader shell with the status
the API gave (401 or 403, passed through), and one island — loaded only in that mode, as a further
runtime branch of the page's single hoisted script — asks again with the session's bearer and
renders the payload through `renderPreview` + `hydratePreview`, the exact pipeline and hydrators
the authoring preview already runs client-side. The sidebar tree is rebuilt from the index the
reader was admitted to, and the library landing adds the reader's private books as their own
group once the session settles — it, too, is rendered from the anonymous index.

**Media is gated with the prose, and fetched by the island.** `/media/{*rest}` finds the file in
mount order as before, and when the source that owns it is private it verifies the bearer and
applies the same 401 / 403 — the check runs only for a private file, so public media stays free
of it. An `<img>` carries no bearer, so the private lesson island fetches every `/media/…`
reference itself and hands the browser a blob URL. A private file is `Cache-Control: private,
no-store`: a shared cache must never hold what the origin showed to one reader.

**The catalog snapshot is keyed on placements as well as content.** A grouping or order edited
from `/admin` is republished on the next tick with no content change; keyed on content alone, the
tree kept grafting the book where it used to be until the repository happened to receive a push.

## Consequences

- **The d2 boards proxy is not gated, and does not need to be:** a private lesson is never
  server-rendered, so its walkthroughs are never compiled into the sidecar's cache — the proxy
  holds nothing of a private book. The figures a reader sees are drawn by the client renderer,
  in their own browser.
- **A private `kind: problem` lesson has no workbench and no Submit** in this version. The judge,
  the tests and the submission history are wired from server-rendered state the shell never had.
  It reads as prose — description and editorial — and the page says so. The study system's boss
  fights already go to the SOURCE lesson's judge, so nothing that exists today depends on it.
- **Figures in a private lesson use the client renderers.** The d2 sidecar draws at SSR, and SSR
  never sees the lesson; the fallback is the same one every lesson gets when the sidecar is absent.
- **The prod `GITHUB_TOKEN` must be able to read the repository.** A fine-grained PAT is granted
  per repository; a private satellite is one more repository on that grant, `contents: read`.
- A private book is otherwise a book: registered from `/admin`, placed by its row, fetched on the
  sixty-second loop, merged first-source-wins. Its slug still comes from `book.json`, because the
  slug is the URL.
