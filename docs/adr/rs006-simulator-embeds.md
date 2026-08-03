# RS006 — Simulators are static bundles content repositories ship and lessons frame

**Status:** accepted · 2026-08-03

## Context

Lessons already carry three kinds of live figures — mermaid and d2 fences rendered client-side,
and LikeC4 views framed from the `/c4` proxy. The next kind is bigger: interactive simulators,
whole self-contained HTML/CSS/JS apps (typically a Vite build: an `index.html` plus hashed
`assets/*`), authored outside this repository and expected to multiply across books.

Nothing could serve one. `/media` maps only `_media/` and its content-type table stops at
images/video/pdf — under the global `X-Content-Type-Options: nosniff`, a module script served as
`application/octet-stream` is refused outright. And a simulator must keep the platform's
publishing property: shipping one is a `git push` to a content repository, never an image rebuild.

## Decision

**A simulator is a directory `_simulators/<sim-id>/` at the root of any content repository,
served at `/simulators/<sim-id>/…`, embedded by a fence.**

### The convention

- `_simulators/<sim-id>/index.html` is the entry point; everything the bundle needs sits beside
  it and is referenced **relatively** (`./assets/…`). No build step runs server-side — the repo
  ships the built artifact.
- The `_` prefix is what keeps the tree invisible to the catalog walker (the same rule `_media/`
  rides), so a simulator never surfaces as an empty chapter.
- `GET /simulators/{*rest}` probes **every mounted source** in mount order, first hit wins — the
  catalog's own duplicate-slug rule, so a bundle being migrated between repositories serves from
  the same source that wins its book. The route holds the LIVE mounted set: registering a
  satellite makes its simulators servable without a redeploy.
- A directory request resolves to its `index.html`; the slashless form 301s to the slash form,
  because the browser resolves the bundle's relative asset URLs against the request path.
- Explicit content types for the bundle vocabulary (html/js/css/json/wasm/fonts/images) —
  `nosniff` makes a wrong type a broken simulator, not a warning. HTML caches for a minute (the
  un-hashed entry point authors replace in place); every other asset for the shared hour.

### The authoring surface

````markdown
```simulator name=osi-encapsulation height=560 title="OSI Encapsulation"
```
````

The fence body stays empty; `name` is the `_simulators/` directory. The renderer emits a marker
div and the reader hydrates it into a same-origin iframe with the diagram family's Enlarge
modal. Bad meta earns the loud authoring-error card with the raw fence kept visible — never a
silently-missing embed. A raw authored `<iframe src="/simulators/…">` also gets the Enlarge
chrome, but the fence is the documented form.

### The trust position

The iframe carries `sandbox="allow-scripts allow-same-origin"`, which for same-origin content is
containment of accidents (popups, top navigation, form posts), not a security boundary. The
boundary remains ADR-S015's: content repositories are first-party — whoever can merge content can
already run script in the page.

## Consequences

- Publishing a simulator is a content push; the app deploys nothing.
- A satellite's simulators exist at the same URL whether the book lives in the spine or its own
  repository — cutovers stay safe under first-source-wins.
- The bundle must be self-contained and relative; an absolute `/assets/…` reference inside a
  bundle breaks silently on this origin, and nothing lints it (accepted for now — the missing
  case surfaces immediately in the embed).
- `/simulators/` is disallowed in robots.txt, like `/c4/` — frames have no standalone audience.
