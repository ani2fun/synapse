# RS012 — A lesson's widgets live beside it, in `_assets/`, gated like the book

**Status:** accepted · 2026-09-25

## Context

RS006 made simulators servable: a directory `_simulators/<sim-id>/` at the ROOT of a content
repository, served at `/simulators/…`. That suits a large, reusable app (the OSI simulator). It
suits a book's figure widgets badly. A book with hundreds of lesson figures would need one flat
root directory per figure, far from the lesson it belongs to, and a build to keep the two in step.
Authors then go back and forth between a lesson folder and a repository-wide tree.

`/simulators` is also ungated. RS011 gates a private book's prose and its `/media`; its simulators
would be readable by anyone who guessed a name.

## Decision

**A lesson may keep its widgets in an `_assets/_simulators/` folder beside the lesson file (or at
its book's root), served at `/content-assets/{slug path}/_assets/_simulators/…` and gated exactly
like the book.**

### The convention

```
<lesson folder>/
  01-some-lesson.md          ```simulator src=_assets/_simulators/index.html?fig=04 height=240 title="…"
  _assets/
    _simulators/             served
    _diagrams/               the figure sources: NOT served
<book folder>/_assets/_simulators/   shared by every lesson of the book (a runtime, a stylesheet)
```

- The `_` prefix keeps `_assets/` out of the catalog walk, as it does `_media/`.
- The slug path is the reader URL's: `/synapse/{slug path}` for the lesson, and just the book's
  slug path for the book folder. The catalog resolves it to the lesson file's folder
  (`lesson_files`) or the book's folder (`book_dirs`, recorded by the walker). A slug path that is
  neither is a 404.
- **Only `_assets/_simulators/` is served.** Diagram sources, and anything else a lesson keeps in
  `_assets/`, stay unpublished. The realpath guard is per folder: the target must stay under
  `<folder>/_assets/_simulators`.
- A directory resolves to its `index.html`, and the slashless form 301s to the slash form, as for
  `/simulators`.

### The authoring surface

The `simulator` fence takes `src=` as the alternative to `name=`. `src` must be a relative path
inside `_assets/_simulators/`, with an optional query, and no `..`. The client resolves it against
the page it is on (`/synapse/{slug path}`). Off a reader page, the authoring preview for example,
there is no lesson to resolve against, and the card says so.

### The trust position: gated like the book

- **Public book.** The files are public. They are served `public, max-age=60`, and the iframe
  loads the resolved URL directly.
- **Private book.** The files go to the book's reader list only: 401 to an anonymous caller, 403
  to a signed-in caller not on the list, and `private, no-store` to a reader. The check is RS011's
  `/media` gate, keyed on the owning source's audience.
- **Why a private widget is fetched by the page.** An iframe cannot send a bearer. So on a
  private book the reader page fetches the widget's HTML, and every same-origin `<script src>` and
  stylesheet it references, with the bearer (`lib/privateMedia`). It inlines them in order and
  frames the result as one `srcdoc` (`lib/simulatorDoc`).
- **The page contract.** A `srcdoc` has no query string. So the page defines
  `window.synapseSimulator = { params }` (the `src` query, parsed) before any of the widget's own
  scripts. Widget pages read `window.synapseSimulator?.params ?? new URLSearchParams(location.search)`.
- A 404 is `no-store` on both routes' behalf: a CDN that caches a miss keeps it for hours, and
  the file it hides is usually one pushed a minute later. Cloudflare did exactly that to a
  `/simulators` asset on 2026-09-25.

## Consequences

- A lesson, its figure sources and its widgets sit in one folder, and there is no build step
  between them.
- A private book's widgets are exactly as private as its prose.
- On a private book, a widget that loads more files at run time (a `fetch`, a dynamic `import`,
  a worker) is not supported: only what the HTML references statically is inlined. Such a widget
  belongs in `_simulators/` under RS006, and so does anything meant to be shared across books.
- The slug path in the URL comes from the catalog, so a renamed lesson moves its widget URL with
  it, as it does its reader URL.
