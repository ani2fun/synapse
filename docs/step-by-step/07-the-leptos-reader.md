# Step 07 — The Leptos reader: library, sidebar, lesson

*(oracle: synapse step 07 + the browse slices of 12–13 — `Page.scala`, `AppRouter`,
`CatalogStore`, the library and lesson pages)*

## The app-map (`router/page.rs`)

`Page` is a PURE sum type — `Library` (`/`), `Lesson(path)` (`/synapse/{dir-mirror}`),
`NotFound` — with `from_segments`/`url`/`segments_of` unit-tested natively, exactly the oracle's
testable-routing rule (ADR-S014). The Leptos router in `shell` is a thin location → segments
feeder; pages join the enum as their steps land.

## The three layers (`catalog/`)

- **`logic/`** (pure, purity-gated, native-tested): `reading_order` (pre-order full paths — the
  sidebar's and library card's source of truth), `first_lesson_path` (where a book cover
  points), `book_of` (which book owns a lesson path).
- **`state/`** — `CatalogStore`, an app-level store in Leptos CONTEXT: the index is fetched once
  and shared (library page + every lesson's sidebar), the cache re-arms on failure so a
  transient miss doesn't pin a broken index (oracle semantics). Lessons fetch per navigation.
- **`view/`** — `LibraryPage` (category sections + book cards → first lesson) and the reader:
  `Sidebar` (owning book's reading order, memoized per book, **fine-grained per-item `current`
  tracking** so lesson→lesson navigation flips two classes instead of re-rendering the tree) +
  `LessonBody` (breadcrumb, frontmatter title/summary, the body across the markdown island,
  prev/next from the payload's full paths).

`api/` decodes the SHARED wire DTOs and surfaces the `ApiError` envelope's message on non-200s.

## The bug the browser verify caught

The first cut cached the index signal in a `thread_local` — created under the LIBRARY page's
reactive owner, it went inert when that page unmounted: lesson→lesson navigation updated the
body but the sidebar highlight froze. Leptos rule, now designed in: **app-level stores are
created under `App`'s owner and provided as context** — never under whichever page touched them
first. (The fix also brought the fine-grained per-item tracking.)

## Verified

75 tests green (70 server + 5 client native: app-map + logic); clippy `-D warnings`;
purity/caps/fmt; bundle 225 KiB gz / 700 budget (router + reader ≈ +54 KiB). In-browser against
the REAL synapse-content: the library lists the five production books with first-lesson links;
the DSA reader shows the 33-lesson reading order; SPA navigation updates URL, body, prev/next,
AND the sidebar highlight; a frontmatter-only problem lesson renders an empty body by design
(its workbench arrives in RS-P5); zero console errors.
