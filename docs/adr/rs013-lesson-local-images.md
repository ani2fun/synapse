# RS013 — A lesson's images live beside it, in `_assets/`, gated like the book

**Status:** accepted · 2026-10-29

## Context

Book images live in `_media/<book>/<lesson>/` at the root of the content repository (A4 of every
book charter), referenced as `/media/<book>/<lesson>/file.png`. For a book with screenshots and
diagram exports in most lessons, that splits each lesson in two: the prose in its chapter folder,
its images in a parallel tree, joined by a hand-typed path that breaks when either side moves.

RS012 already gives each lesson an `_assets/` folder and a route, `/content-assets/{slug path}/…`,
resolved through the catalog and gated like the book. It only publishes `_simulators/`.

## Decision

**A lesson may keep its images in the `_assets/` folder beside it and reference them relatively,
as `![alt](_assets/shot.png)`. The RS012 route serves them.**

### What is served

- Under RS012, `_assets/_simulators/` is served whole, as before.
- **New:** a file with an image extension (svg, png, webp, jpg, jpeg, gif) directly in `_assets/`
  or in a subfolder whose name does not start with `_` or `.`.
- Nothing else. A D2 source beside its exported SVG, a `.md` note, an `_drafts/` folder: all stay
  unpublished. The rule is one function, `catalog::domain::lesson_assets::is_public_image`, read by
  the route and by the lint, so they cannot disagree.
- The realpath guard is per kind: an image must resolve under `<folder>/_assets`, so a symlink
  named `x.png` cannot reach the lesson's prose or anything outside. A directory is never an image.

### The authoring surface

- The renderer (`renderLesson(raw, { slugPath })`) rewrites an image src of `_assets/…` or
  `./_assets/…` (no `..`) to `/content-assets/{slug path}/_assets/…` for the page being rendered.
  It runs before the frame-run pass, so a stepping figure built from lesson-local frames works too.
- The reader page and the private-lesson island pass the slug path. A caller with no page behind
  it (the editor's review preview, practice panes) passes none, and the src stays as written.
- `validate-book` errors on a lesson-local image whose file is missing, or whose path the server
  will never serve.

### The trust position

Unchanged from RS012. A public book's images are `public, max-age=60`. A private book's go to its
reader list only (401 / 403, `private, no-store`), and the private-lesson island fetches them with
the bearer and swaps in a blob URL (`lib/privateMedia` now resolves `/content-assets/` as well as
`/media/`). A miss is `no-store`.

## Consequences

- A lesson, its screenshots and its diagram sources sit in one folder and move together.
- `_media/` stays valid; nothing migrates. New books should prefer `_assets/`.
- The editor's review preview shows a lesson-local image as broken, because it has no slug path.
  It can learn one later from the lesson it is editing.
