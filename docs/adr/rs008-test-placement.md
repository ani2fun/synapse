# RS008 — A module's tests live at its natural child path, and are not measured as production

**Status:** accepted · 2026-08-25

## Context

Rust offers three places for a test and the Book names all three: a unit test inline in the file it
tests, an integration test under `tests/` at the crate root, and a doc test on a public item. Only
the first is a real choice — `tests/` is a SEPARATE CRATE that sees only `pub` items, so a test of
anything private has to stay in-crate, and no arrangement of directories changes that. There is no
`src/main` / `src/test` split to reach for; Java's works because `src/test/java` recompiles into
the same package, and Rust privacy is crate-internal and compile-time.

Inline is the overwhelming default: across the 415 crates in a local cargo registry, 1150 inline
`#[cfg(test)] mod tests { … }` blocks against 115 declarations of a separate file. This repository
had drifted into a third shape — a flat `<module>_tests.rs` sibling reached by `#[path]` — used by
2 of those 415.

Two gates here make the choice consequential in a way it is not for a typical crate:

- `check-conventions.sh` caps a file's LINES, counting tests. Test prose therefore spent the
  production budget: `config.rs` was 387 lines, 124 of them tests.
- `coverage.sh` separates test code from production so the 88% floor measures the code that ships.
  It did so with the filename regex `_tests\.rs$`.

A filename is a guess about intent, and this one had guessed wrong three ways simultaneously,
none of which fails a build:

| File | Actually | The regex said |
|---|---|---|
| `problem_tests.rs` | production — the `FsProblemTests` adapter | test → **excluded from the floor** |
| `judge_vectors_test.rs` | test code (singular `_test`) | production → 49 always-hit lines counted |
| `merge_fixtures.rs` | test support | production → 141 always-hit lines counted |

Inline blocks were the same fault in a fourth shape: no regex over names can see inside a file.

`#[path]` also proved viral. A `#[path]`-loaded file resolves ITS children beside the parent rather
than beside itself, so nesting a submodule inside a suite needed a second `#[path]` and a comment
explaining why. The flat layout additionally forced fixtures to sit BESIDE the suites that used
them, because a fixture module nested in one suite is invisible to its sibling — a constraint the
code documented and worked around.

## Decision

A module's tests live at its natural child path.

```
catalog/domain/merge.rs            the module
catalog/domain/merge/tests.rs      #[cfg(test)] mod tests;
catalog/domain/merge/tests/        everything the suite needs
  fixtures.rs                        mod fixtures;
  order.rs                           mod order;
```

`foo/mod.rs` is tested by `foo/tests.rs` the same way. Black-box suites stay in
`server/tests/*_it.rs`, which is the genuine `src/test/java` equivalent and was never in question.

Three rules follow:

- **No inline blocks.** Test lines do not live in a production file.
- **No flat `*_tests.rs` siblings.** The old shape, and the one whose name had to be trusted.
- **No `#[path]`, ever.** Rust resolves this layout unaided. `#[path]` exists only to defeat that
  resolution, and one use forces the next.

Because `order` is a CHILD of `tests` rather than its sibling, it sees its ancestors' private
items — so fixtures simply live inside the suite, and the workaround the flat layout required
disappears rather than being reimplemented.

### The coverage boundary becomes a path

`coverage.sh` excludes `/tests\.rs$|/tests/` and nothing else. This is the point of the shape: a
production file cannot land on that pattern by accident the way it can land on a name, and
everything a suite loads — fixtures, fakes, topical sub-suites — is excluded structurally, with no
per-file special cases to keep in sync. The previous `service_fakes.rs` exception is gone.

### Test files are exempt from the line caps

A suite is a list. It grows with the surface it covers, and splitting one to fit a budget invents
module boundaries that describe nothing — one sub-suite here cited the cap as half its reason to
exist. The caps still bind production code in every tier.

## Consequences

- `check-conventions.sh` §5 enforces the layout across `server/src`, `shared/src` and
  `viz-wasm/src`, rejecting five things: an inline block, a flat sibling, any `#[path]`, a
  `tests.rs` its owner never declares, and a file under `tests/` its root never names. The last two
  are the silent failures — an undeclared test file is not compiled, and a suite that does not run
  reads exactly like a suite that passes.
- A module with tests gains a directory. That is the visible cost, and it is the shape `axum`,
  `hyper`, `chrono`, `base64`, `memchr` and `indexmap` already use.
- The reported coverage number FELL when the fixtures padding it stopped counting. That is the
  gate becoming honest, not a regression, and the floor did not move.

## Alternatives rejected

- **Inline, the ~90% default.** Reintroduces both faults this ADR exists to remove: test lines
  scored as covered production code, and test prose spending the production line budget. The
  majority convention is right for a crate with neither gate.
- **Keeping the flat `_tests.rs` sibling and only widening the regex.** Cheaper, and it fixes
  nothing structural: `merge_fixtures.rs` still needs a name the regex happens to know, and the
  next support file needs another.
- **Moving unit tests to `tests/`.** Buys file separation by making internals `pub` — paying in the
  design for a layout preference.
