# Step 03 — The catalog domain: the walker and its conventions

*(oracle: synapse steps 03–04's domain layer — `SynapseContentWalker`, `Frontmatter`,
`CatalogResolver`, and the content/catalog models; `SynapseContentWalkerSpec` +
`FrontmatterSpec`/`FrontmatterParseSpec` + `CatalogResolverSpec` ported as the spec)*

The reference hexagon walk begins where the oracle began it: with the pure heart of `catalog`.
Everything in `server/src/catalog/domain/` is std + serde only — the purity gate greps it.

## The model (two trees + the map between them)

- **`content_tree.rs`** — the *uninterpreted* on-disk tree the adapter will materialize:
  `ContentEntry::{File, Dir}` with pre-decoded, all-optional `BookMeta` (`book.json`) and
  `CategoryMeta` (`category.json`). Lenient by design (ADR-0001).
- **`catalog.rs`** — the *browsable* result: `CatalogEntry::{Category, Book}` at library level,
  `BookEntry::{Chapter, Lesson}` inside a book (lesson **bodies are not held** — read per
  request, ADR-S010), plus `WalkResult.lesson_files`: per book slug, in-book slug-path → the
  content-root-relative file path with order prefixes and real folder names intact — the bridge
  from pretty URLs back to files.
- **`SynapseContentError`** (thiserror): `DuplicateBookSlug` · `DuplicateLessonSlug` ·
  `MaxChapterDepthExceeded` · `InvalidSlug` — the conventions the walk refuses to paper over.

## The walker (`walker.rs`)

The naming rules, ported exactly and pinned by tests: `slug_like` / `lesson_path_like` (also the
first traversal guard — `..` is not slug-like), `strip_order_prefix` (`01-foo`→`foo`),
`humanise` (`01-singly-linked-list.md`→`Singly Linked List`), `slugify` (`Hello World!`→
`hello-world`). Structure rules: a dir with `book_meta` is a book, else a category (kept only if
a book lives beneath); `_*`/`.*`/non-slug dirs and the reserved aux dirs (`examples`, `c4`,
prefix-stripped) are skipped; `.editorial.md` sidecars are not lessons. Ordering: library levels
sort by `(order ?: MAX, dir name)`; book interiors by `(index-first / numeric prefix, name)`.
Lesson titles resolve frontmatter → first `# ` H1 → humanized filename; `essential` defaults
true. Chapters nest to `MAX_CHAPTER_DEPTH = 6`. An explicit `book.json` slug overrides the
folder-derived one while file paths keep the real folder — pinned by a test.

One Rust-shaped divergence, documented: the oracle collects duplicate lesson slugs and reports
them sorted; the port does the same via `BTreeSet` accumulation during the walk (no second pass).

## Frontmatter (`frontmatter.rs`) and navigation (`resolver.rs`)

The lenient fence: only if the FIRST line is `---` and a terminator follows; anything malformed
degrades to "no fence, whole content is body" — pinned by the unterminated-fence tests.
`fields_and_body` (generic, for sidecars) · `extract_title` · `extract_essential` · `parse`
(typed `LessonFrontmatter`, inline flow-lists for `topics`).

`resolver.rs`: `resolve_lesson` descends categories by slug, stops at the first book, and walks
chapters to a lesson (a chapter path or bare book prefix is `None`); `lessons_in_reading_order`
is the pre-order flatten that prev/next will hang off in step 04.

## Tests

40 unit tests (`walker_tests.rs` 24 · `frontmatter_tests.rs` 10 · `resolver_tests.rs` 4 ·
component-doc 2), each a behavior lifted from the oracle spec. Big suites live in sibling
`*_tests.rs` files (`#[path]` modules) so the 500-line cap keeps measuring code, not test prose.

## Verified

`cargo test` 40/40 green; clippy `-D warnings` (one deliberate allow:
`case_sensitive_file_extension_comparisons` — `.MD` must not silently become a lesson);
purity + caps + fmt green.
