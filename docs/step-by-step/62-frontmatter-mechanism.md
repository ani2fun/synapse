# Step 62 — Mechanism and vocabulary part ways

*(The catalog/blog frontmatter twins were defending the right rule at the wrong line.)*

## The ask

Item 6, the loop's finale. Since step 18 the blog's fence parser has been a deliberate
byte-identical twin of the catalog's — "bounded contexts own their vocabulary (the oracle
duplicated it for the same reason)". The deepening assessment drew a finer line: the
duplication defends a DDD rule, but the rule is about **domain language**, and the fence
splitter isn't language — `fields_and_body`, the quote-stripper, and the inline-list parser
say nothing about lessons or posts. They are generic-subdomain mechanics; the deletion test
on the twin says the copy earns nothing (delete one and it reappears verbatim). What IS
vocabulary — `LessonFrontmatter {title, summary, essential, kind, difficulty, topics}` vs
`BlogPost {title, summary, publishedAt, tags, readMinutes, eyebrow}`, and how each field
degrades — stays exactly where it was, one per context.

## The shape

- **`platform/frontmatter.rs`** (`pub(crate)`): `fields_and_body`, `parse_inline_list`,
  `strip_matching_quotes` — the leniency contract (ADR-0001: malformed fence degrades to
  "no fence", metadata never fails a page) documented once at the mechanism. Catalog's
  `domain/frontmatter.rs` re-exports the pair (`pub(crate) use`), so its consumers
  (`walker`, `component_doc`, the service) compile unchanged; blog imports directly and its
  `inline_list`/`unquote` names retire. Pure text functions — the domain-purity gate is
  untouched, and both context module docs now state the mechanism/vocabulary split instead
  of the twin rule.
- **`platform/blocking.rs`** (`pub(crate)`): the one `run_blocking`
  (`spawn_blocking` + resume-the-unwind, keeping the catalog copy's fuller comment); both
  filesystem adapters import it. Infrastructure-side by nature (tokio), so `platform/` —
  never under a `domain/`.
- **The watermark pair stays per-context, on purpose.** It looked like a twin and is not:
  catalog walks recursively with hidden-subtree pruning; blog counts a flat post listing.
  Same output contract (`"<newest mtime ms>:<count>"`, `"0:0"` degraded), different
  traversal — a shared implementation would have to grow flags for what is honestly two
  functions.

## Verified

Conventions · fmt · clippy pedantic (clean) · `cargo test --workspace` 458 · vitest 83.
The decisive check: catalog's frontmatter/walker suites and blog's domain suite all pass
**unmodified** — they drive the per-context public functions, which is exactly why the
mechanism could move without any test noticing. Net: two ~40-line verbatim copies retired;
the twinning rule survives where it says something.
