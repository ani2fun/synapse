# Step 08 — The markdown pipeline: the oracle's renderer, verbatim

*(oracle: synapse step 08 — `render.ts` + `render.test.ts` + `d2.ts`, ADR-S015; the vitest
suite ported as the spec)*

## Reused verbatim (the whole point of the TS islands)

`render.ts` (unified → remark-gfm → remark-rehype → rehype-slug → rehype-pretty-code/shiki →
rehype-stringify, with the run-fence → workbench grouping, `testcases`/`quiz` JSON fences,
solution spoilers, mermaid placeholders, and parse-time d2) and `d2.ts` are **copied unchanged**
from the oracle, with its full 40-test vitest suite — which passed unmodified on the first run.
The suite runs via `npm test` and joined CI.

`loader.ts` keeps the oracle's chunk-splitting shape (tiny eager loader, dynamic-imported
renderer) with one documented deviation: it exports `renderMarkdown(src)` directly rather than
`loadRenderLesson()` returning a function — the friendlier wasm-bindgen FFI shape; same caching,
same chunking. The reader's `lesson-body` gains `.synapse-prose`; the shiki dark-slab CSS
variables are verbatim oracle values, and interactive placeholders (workbench/quiz/solution)
render as visible dashed seams until their steps mount real widgets.

## Three findings the step banked

1. **Islands must live under the npm root.** The planned top-level `client-ts/` broke bare
   specifiers (`unified`, `@terrastruct/d2`) — Node resolution never reaches
   `client/node_modules` from a sibling tree. The islands moved to `client/islands/` (the
   oracle keeps them in `client/src` for the same reason); the `@markdown` alias is unchanged,
   so the Rust externs never noticed.
2. **wasm-pack got replaced.** It repeatedly died on its own tool discovery ("invalid type:
   map, expected a string") even with clean caches. `dev-tools/build-wasm.sh` now spells out
   the same three steps — `cargo build --target wasm32` → `wasm-bindgen` (version-checked
   against Cargo.lock) → `wasm-opt -Oz` on release — readable, deterministic, used by npm
   scripts and CI (taiki-e installs wasm-bindgen; binaryen from apt/brew).
3. **The budget gate had a false-positive glob.** d2's lazy 8 MB chunk is *named* `index-*.js`
   (npm entry naming) and got counted as critical path. The gate now sums exactly what
   `index.html` references plus the app wasm — lazy chunks (d2, shiki grammars, the 541 KB
   render pipeline itself) stay off the path BY DESIGN, which is the loader pattern working.

## Verified

75 Rust tests + 40 vitest green; clippy/fmt/purity/caps green; release build with wasm-opt;
critical path **226 KiB gz / 700** (the pipeline added ~1 KiB — everything heavy is lazy).
In-browser against real content in a clean tab: a 75-code-block lesson renders with 2,700 shiki
token spans and 112 slugged headings; a problem lesson's run-fence group collapses into ONE
workbench placeholder; lesson→lesson→library SPA navigation with zero console errors. (A dirty
long-lived dev tab showed cross-instance disposed-signal noise from earlier wasm instances —
fresh-tab verification is the honest measure; noted for the e2e checklist.)
