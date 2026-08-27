# Multi-board d2 — walkthroughs a reader drives

A d2 source that uses `layers:` is not one picture, it is a **tree of boards**: a C4 stack, a
zoom-in, a system explained one level at a time. d2 ships one way to show all of them —
`--animate-interval`, which cycles on a timer nobody can stop.

This gives the reader the wheel instead. Click a node carrying `link:` to drill into that board;
step back out with ◀ ▶ ⌂, jump with a menu, share the board you are looking at.

Three surfaces, one model:

| | |
|---|---|
| **In a lesson** | ```` ```d2 boards ```` in any markdown → a navigable figure, drawn on demand |
| **`/d2`** | An editor: source on the left, the live walkthrough on the right |
| **`d2-interactive`** | One `.d2` → one self-contained `.html` that works over `file://` |

---

## The fence

````markdown
```d2 boards name="url-shortener" root="System Context"
shortener: "URL Shortener" {
  link: layers.container
}

layers: {
  container: {
    api: "Public API" {
      link: _.layers.component
    }
  }
  component: {
    handler: "Redirect Handler"
  }
}
```
````

| Marker | Meaning |
|---|---|
| `boards` | Required. Opts this fence into the walkthrough viewer. Without it a `layers:` diagram renders its root board only, and the links are dead. |
| `name="…"` | A label for the walkthrough, used by `/d2` for its title and its export filename. It no longer names anything on disk — the figures are content-addressed — so it is documentation, not addressing. |
| `root="…"` | The root board's title. A layer's title comes from its key; the root has no key, so it takes this (default `Overview`). |

A `boards` fence never joins a slideshow: adjacent ```` ```d2 ```` fences group into a stepper,
and a walkthrough always stands alone.

---

## `link:` — the one thing that trips everyone

**`link:` resolves against the board it is written in.** At the root, `link: layers.container`
means `root.layers.container` and is correct. One level down, inside `layers.container`, the same
text means `root.layers.container.layers.component` — which does not exist, so **d2 drops the
link, emits no anchor, and still reports success**. `d2 validate` says nothing. The node simply
stops being clickable.

```d2
layers: {
  container: {
    api: "Public API" {
      link: layers.component     # ✗ silently dropped
      link: _.layers.component   # ✓ `_` steps up to the parent board
      link: root.layers.component # ✓ absolute, also fine
    }
  }
}
```

Because the compiler keeps no trace of a dropped link, this is caught by reading the **source**:
`d2-interactive` reports every one, with the file, the line, the board it
sits in, and the fix:

```
01-intro.md:33: `link: layers.component` in board root.layers.container names no board
  — did you mean `link: _.layers.component`?
```

It warns and carries on; the node renders, it just does not navigate. `--strict` turns the
warning into a failure.

**External links** (`https://…`, `mailto:…`) are left alone and open in a new tab.

---

## Where the figures come from

Nowhere on disk. A walkthrough is drawn ON DEMAND by the `d2-render` sidecar and cached by
content (ADR-RS009) — there is no artifact in the content repository, nothing to commit, and no
CI step between writing a fence and seeing it.

Rendering a lesson, the page tier POSTs the fence to `/boards`. The sidecar compiles it once,
draws **every** board into its cache, and returns the graph plus the root. The root is inlined
into the HTML; the reader fetches the others from `/api/synapse/d2/{hash}/{slug}` as they click,
which are cache reads rather than renders.

`{hash}` is `fnv1a` of the fence source, so a walkthrough is addressed by what it IS: the same
diagram in two lessons is one set of boards, and moving a lesson changes nothing. That is what
retired the `_d2/<fence>/` sidecars, `boards.json` on disk, and the `?lesson=` query the old
route needed.

Ordinary single-board ```` ```d2 ```` fences take the same path through `/render`.

### When the sidecar is not there

Nothing breaks: with `SYNAPSE_D2_RENDER_URL` unset, or the sidecar down, slow, or handed a
diagram it cannot parse, the page ships the source and the browser draws the whole walkthrough
itself. Slower, identical behaviour, and the same floor the authoring preview has always used.
The reader also drops a manifest whose recorded source hash does not match the fence, rather
than confidently serving a different diagram.

---

## `d2-interactive` — the standalone page

```bash
node dev-tools/d2-interactive.mjs dev-tools/examples/url-shortener.d2 \
  --output url-shortener.html --title "URL Shortener"
```

| Flag | |
|---|---|
| `--output` | Defaults to `<input>.html` beside the source. |
| `--title` | The page title, and the root board's title. |

Run it from wherever `@terrastruct/d2` is installed — like `render-d2.mjs`, it resolves the
engine from the **working directory**, so it can live beside the diagrams rather than beside an
install.

The output is **one file**: every board, the stylesheet and the viewer are inlined. No server, no
CDN, no build. Open it, mail it, commit it.

This is the one place the diagram owns the browser's Back button — `pushState` per board,
`popstate`, and `#<slug>` deep links — because nothing else is on the page for Back to mean.
Inside a lesson that would be hostile, so there the viewer keeps its own history and writes
`?board=<slug>` with `replaceState`: shareable, and Back still leaves the page in one press.

---

## `/d2` — the editor

Source on the left, the live walkthrough on the right. The preview is the **same component the
lesson page mounts**, so clicking a node drills down and Enlarge opens the same overlay a reader
gets — what you drive is what ships.

Everything stays in the browser (the draft autosaves to `localStorage`) until you choose to send
it somewhere:

- **Copy fence** — the ready-to-paste ```` ```d2 boards ```` block.
- **Download .d2** — the raw source.
- **Export boards** — a zip of the board graph plus one SVG per board. Nothing in the app reads
  these any more; it is for taking the drawn figures somewhere else (a slide, a README, a review).
- **Add to a lesson…** — inserts the fence into a lesson and opens a pull request, through the
  existing content-editing pipeline. Needs sign-in and the content-editor allowlist. A second
  submission for the same lesson while its PR is open becomes another **commit on that PR**.
  `local-only-content/` is never editable, so those lessons are not offered.

---

## Supported, and not

**Supported.** `layers:` to any depth; `link:` between boards; external links; folder-only boards
(skipped, their children reparent to the nearest board that renders); every d2 shape, style,
class and icon — the SVG is exactly what `d2 render` produced, never rewritten.

**Not.** `steps:` and `scenarios:` are drawn but not navigable — they are sequences, and adjacent
```` ```d2 ```` fences already make a stepper. A `boards` fence carrying them warns.

Board titles come from the layer key, title-cased (`redirect_handler` → `Redirect Handler`). d2
has no board-label keyword to read.

**Browsers.** Chromium and Firefox, current versions, desktop and mobile. The standalone page
needs no network at all; the in-lesson viewer fetches one small SVG per board as you navigate,
and never the 5.9 MB engine.

---

## How it hangs together

```
```d2 boards fence
   │
   ├─ SSR ────────── POST /boards → the sidecar draws EVERY board, cached by
   │                 content; inlines the ROOT, ships the graph in `data-boards`
   │
   ├─ client ─────── D2Boards.tsx: click → closest("a") → href IS the board id
   │                 siblings fetched from /api/synapse/d2/<hash>/<slug>
   │
   └─ CLI ────────── dev-tools/d2-interactive.mjs → one .html, all boards inlined
```

Nothing rewrites the SVG. d2 writes the absolute board path into every anchor it emits
(`href="root.layers.container"`), so navigation is a lookup against the manifest — which leaves
the figure byte-for-byte what the engine produced, and leaves the anchors real links: focusable,
Enter-activatable, announced as links.

The board model — ids, slugs, salts, titles, the manifest — lives once, in
`dev-tools/d2-boards.mjs`, and is inlined into the standalone page rather than reimplemented.
The app's TypeScript copy (`web/src/lib/islands/diagram/boards.ts`) is pinned against it
byte-for-byte by `renderD2Script.test.ts`, because a slug that disagrees by one character misses
every lookup silently.
