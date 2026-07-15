# Step 06 — The catalog HTTP layer: three endpoints and the wire shape

*(oracle: synapse step 06 + the ADR-S033 cache forward note — `CatalogApi.scala` DTOs,
`CatalogRoutes`, `ContentCacheControl`; `CatalogRoutesSpec` ported as wire-pinning ITs)*

## The wire contract (`shared/src/catalog.rs`)

The kind-discriminated tree, field names load-bearing: `CatalogEntryDto` tags `"kind":
"category" | "book"`, `BookEntryDto` tags `"chapter" | "lesson"` (serde internally-tagged enums —
the exact shape circe's discriminator produced). camelCase fields (`categoryPath`,
`estimatedReadingMinutes`); Options serialize as nulls (circe parity). `LessonPayloadDto.raw` is
the fence-stripped markdown; `prev`/`next` are ready-to-navigate FULL paths. Recursive schemas
carry `#[schema(no_recursion)]` for utoipa.

## The endpoints (`catalog/http/`)

`routes.rs` — registration mirrors the oracle's specificity ordering, which axum's router
enforces structurally: `/api/synapse/index` (static) · `/api/synapse/c4-doc/{element_id}?lesson=`
(static segment routes dotted FQNs past the catch-all — pinned by an IT) ·
`/api/synapse/{*paths}` (the lesson catch-all). `dto.rs` maps domain → wire ONLY here; prev/next
become full paths (`categoryPath + bookSlug + "/" + inBook`); `to_error`: `NotFound`→404,
`Io`/`IndexInvalid`→500, always the `ApiError` envelope.

The http layer is CONCRETE over `CatalogService<FileSystemContentRepository>`
(`LiveCatalogService`) — the port's genericity serves the application tests; nothing varies at
runtime here, and it keeps utoipa's path derives plain.

## The cache header (`platform/content_cache_control.rs`)

`public, max-age=60, stale-while-revalidate=600` — GETs only, **200s only** (a cached error is a
poisoned edge), and only `/api/synapse|/api/blog` paths; never `/api/health|me|auth|run|…`.
Applied as one layer over the whole surface in `app()`; ITs pin present-on-200,
absent-on-404/500, absent-on-health.

## Config: `SYNAPSE_ROOT` (and the bug the smoke test caught)

`content_root` (+ `auto_reload`) join `AppConfig`; `SYNAPSE_ROOT` — the oracle's env name — maps
onto the field via a figment key map. The first attempt used a serde alias, which **crashed at
boot** ("duplicate field") because the serialized default and the env key both feed the merged
figment dict; caught by running the real binary, fixed, and pinned by a unit test.

## Verified

70 tests green (53 unit incl. the `SYNAPSE_ROOT` pin · 6 route ITs · 9 FS ITs · 2 contract ·
2 health, minus overlaps); clippy/fmt/purity/caps green. **Smoke against the REAL
synapse-content checkout**: the index lists the production books (synapse-features,
system-design-from-first-principles, low-level-design, dsa, …) with the cache header stamped,
and a real lesson (`synapse-features/reading-a-lesson/reading-a-lesson`) serves with frontmatter
title, body, and a correct `next` pointer. Insomnia collection grew the three requests in the
same step, per the standing rule.
