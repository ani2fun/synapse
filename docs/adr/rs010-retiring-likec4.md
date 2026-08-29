# RS010 — LikeC4 is retired; D2 draws every diagram

**Status:** accepted · 2026-08-28 · **amends [RS005](rs005-multi-repo-content.md)** (its merged-C4
section) · **completes the direction set by [RS009](rs009-on-demand-d2.md)**

## Context

Two diagram systems have been running side by side.

**LikeC4** was the older one. `.c4` model files lived beside the prose that embedded them; a CI
workflow compiled every `.c4` in the catalog into one merged SPA, pushed it as a container image,
promoted the tag into the infrastructure repository, and let ArgoCD roll it out. The application
reverse-proxied that pod at `/c4`, lessons embedded it as a raw `<iframe src="/c4/view/<id>">`, and
each element could carry a tutorial doc at `_c4-docs/<elementId>.md` that slid in from the right
when a reader clicked its box.

**D2** is the newer one. A fence in the lesson is drawn on demand by the `d2-render` sidecar and
inlined at SSR, content-addressed, with nothing generated at publish time and nothing committed
(RS009). A ```d2 boards fence compiles to a *tree* of boards the reader clicks down through.

By 2026-08 the second had subsumed the first. `01-url-shortener.md` carried the comparison in the
prose: one boards fence covered context → containers → code where LikeC4 needed two embeds and a
running service, and the lesson already recorded the conclusion — *"the walkthrough is how new case
studies will be drawn."*

### What LikeC4 actually cost

- **A second publishing speed.** Prose reached production in under a minute through the git-sync
  sidecar. A diagram took an image build, a registry push, a promotion commit and a pod rollout.
  Two paths was the standing answer to "why hasn't my change appeared?", and the content book had a
  chapter explaining it.
- **A shared global namespace.** One workspace, one `specification {}`, view ids unique across
  every book in every repository. RS005 had to gather `.c4` files from the registry to keep it
  merged, which meant an unauthenticated `/api/c4/sources` endpoint existing solely so a CI job
  with no token could ask the running application what to check out.
- **Two silent failure modes.** A satellite's `.c4` rendered nothing until the gathering build was
  deployed, and the iframe answered 200 with LikeC4's own error panel — so HTTP status proved
  nothing. Worse in the other direction: both copies of a book's model in one workspace, which
  `likec4 build` accepted while silently resolving 390 duplicate-element and duplicate-view
  collisions. Only `likec4 validate` caught it.
- **Prose nobody could find.** 198 `_c4-docs/*.md` write-ups — roughly 290 KB of good material —
  were reachable only by clicking a box in an iframe. Invisible to search, to the sitemap, and to
  every reader who did not think to click.

## Decision

**Retire LikeC4 entirely. Every architecture diagram becomes a `d2 boards` walkthrough in the
lesson that shows it, and every `_c4-docs` write-up becomes prose in that same lesson.**

- 17 `.c4` models → 14 walkthroughs plus one plain figure, converted by a throwaway script against
  the house class vocabulary in `synapse-features/_d2-blocks/lib/theme.d2` (`client` / `edge` /
  `svc` / `data` / `async`, plus `external`), which already encoded the same five tiers the LikeC4
  `specification {}` encoded as category colours.
- 198 sidecars → a `## 🧱 Component reference` section per lesson, collapsed behind a `<details>`
  and ordered the way the model declares its elements, which is the order a reader walks the
  boards. The section's own heading stays outside the `<details>`, so the page outline carries one
  row for it rather than eleven rows pointing into collapsed content — `islands/chrome.ts` skips
  headings inside a `<details>` for exactly that reason.
- 31 `<iframe src="/c4/view/…">` embeds → 0.

### What is deleted

| | |
|---|---|
| infra | the `synapse-likec4` and `likec4` Deployments, their Services, their ArgoCD Applications, two NetworkPolicies |
| content | `likec4.Dockerfile`, `likec4-build-push-promote.yml`, `_build-push-promote.yml` (its only caller), `.dockerignore`, `c4-sources.json` |
| server | `platform/likec4_proxy.rs`, `catalog/http/c4.rs`, `catalog/domain/component_doc.rs`, `GET /api/synapse/c4-doc/{id}`, `GET /api/c4/sources`, `ComponentDocDto`, `C4SourceDto`, `LIKEC4_URL`, the satellite `specification {}` / view-prefix lint |
| web | `C4Embed.tsx`, `C4DocsPanel.tsx`, `c4Store.ts`, `resolveC4Node`, the `/c4` dev proxy, ~64 lines of CSS |
| repo | the `c4` compose profile and its service, `Disallow: /c4/` in robots.txt |

The spine now builds **no image at all**: it has no Dockerfile and no workflow that produces one.
Content is data on every path.

## Consequences

**What is lost, plainly.** Per-element aim. Clicking *Base62Codec* used to open *Base62Codec*;
now one disclosure opens all eleven and finding the right one is a scan. LikeC4's relationship and
details tools go too. What replaces the panel is not the page getting longer — the reference is
collapsed, so a reader who does not want it sees one line — but the words being *in the document*:
indexed, linkable, printable, and impossible to orphan by taking a service down.

**Lessons grew on disk, not on screen.** The 14 case studies went from ~45 KB to ~67 KB of
Markdown each — the same prose that was always there, now in the document rather than beside it.
Collapsed, it costs a reader one line.

**One namespace fewer.** A walkthrough is addressed by the hash of its source, so nothing is
global, nothing collides, and a satellite's diagrams work the day it is registered. RS005's
migration precondition loses its diagram clause entirely: there is no gathering build to deploy
first, and no duplicate-workspace window during a cutover.

**One silent failure mode remains, and it is the one RS009 chose.** A sidecar that is down, slow
past the 5 s budget, or handed a diagram it cannot parse falls back to the client renderer: the
page still renders, 5.9 MB heavier. `e2e/tests/d2-prerender.spec.ts` asserts both modes, each
asserting the other's markers are absent.

**The `link:` landmine is now the only diagram rule worth memorising.** `link:` resolves against the
board it is written in — `layers.container` at the root, `_.layers.container` one level down. A link
that lands nowhere is silent in every direction: `d2 validate` reports success, the board draws, and
the reader clicks a box that does nothing.

## Alternatives considered

**Keep the docs panel, retarget it to d2 node clicks.** A `link: "doc:rusSys"` scheme would have
worked — `D2Boards` already routes scheme-carrying hrefs — and would have needed almost no content
rewriting. Rejected: it preserves the panel's real defect, which is that the text is invisible until
clicked, and it keeps ~350 lines of code plus an endpoint alive to serve prose that belongs in the
lesson.

**Keep LikeC4 for the C4-shaped diagrams only.** Rejected: the cost was never per-diagram. It was
the image, the workflow, the two pods, the proxy, the shared namespace and the second publishing
speed — all of which are paid in full for one `.c4` file or for fifty.
