// ──────────────────────────────────────────────────────────────────
// D2 RENDERER — ```d2 fence source → SVG string, via @terrastruct/d2
// ──────────────────────────────────────────────────────────────────
// d2, like mermaid, is a self-contained declarative-diagram renderer (ADR-S026, orthogonal to
// the viz engine). `@terrastruct/d2` resolves per environment — the browser build in the client,
// `dist/node-esm` under SSR — so this one module serves both sides and a server-rendered lesson
// is byte-identical to what the client would have produced from the same source and salt.
//
// ONE instance, and every compile+render serialised through it. A `D2` is not a cheap handle:
// constructing one spawns a worker and instantiates a ~21 MB WASM module, and in the browser it
// first base64-decodes and brotli-decompresses that binary ON THE MAIN THREAD. A page holding N
// diagrams must therefore construct ONE, not N.
//
// Sharing is what forces the queue. The worker binding tracks a SINGLE in-flight request — one
// resolve/reject pair, overwritten on every send — so two overlapping calls clobber each other's
// continuation and both hang. `enqueue` is the precondition for sharing an instance at all, not
// a throughput tweak layered on top of it.

import { fnv1a } from "../../hash";
import * as log from "../../log";

type D2Module = typeof import("@terrastruct/d2");
type D2Instance = InstanceType<D2Module["D2"]>;

// The two halves of the render contract. `dev-tools/render-d2.mjs` draws a content repo's
// diagrams ahead of time and must produce byte-identical output, or every lookup misses and the
// page quietly falls back to compiling here — so both are exported and pinned by a test.

/** dagre or elk. Both ship in the worker — it evals `elk.js` at init either way, so the choice
 *  costs nothing in bundle size or startup, and measures the same on this catalog's diagrams. */
export const LAYOUT = "elk";

/**
 * Always the light neutral theme (themeID 0), independent of the reader's page theme: authored
 * diagrams color nodes with a fixed *light* pastel palette and never set a label text color, so
 * the theme default supplies it — a dark theme would paint light text on light fills and become
 * unreadable. Light-theme text reads on every fill; the SVG sits on a light "card" (diagrams.css).
 */
export const d2RenderOptions = (salt: string) => ({
  themeID: 0, // neutral default — dark text, reads on the authored light fills
  pad: 20,
  noXMLTag: true, // embedding into HTML, not writing a file
  salt,
});

// ── THE SHARED INSTANCE ──────────────────────────────────────────

let instance: Promise<D2Instance> | null = null;

function d2(): Promise<D2Instance> {
  if (instance == null) {
    const started = Date.now();
    instance = import("@terrastruct/d2").then((mod) => {
      log.debug(`d2: worker + wasm booting (${Date.now() - started} ms to module)`);
      return new mod.D2();
    });
  }
  return instance;
}

// ── THE QUEUE ────────────────────────────────────────────────────

let queue: Promise<unknown> = Promise.resolve();

/** Run `job` after every job queued before it. A rejection reaches its own caller and leaves
 *  the queue runnable — one malformed diagram must not strand every later one. */
function enqueue<T>(job: () => Promise<T>): Promise<T> {
  const result = queue.then(job);
  queue = result.catch(() => undefined);
  return result;
}

// ── SALT ─────────────────────────────────────────────────────────

/**
 * The suffix d2 appends to every id inside one SVG, so several diagrams coexist in one document.
 *
 * Derived from the SOURCE rather than a counter, which buys two things: the same diagram renders
 * identically wherever it appears, so a cache keyed on the salt hits across pages and across
 * requests; and SSR output is stable, so the same lesson serves the same bytes every time.
 * `seen` disambiguates the rare case of one document repeating a diagram verbatim — pass one
 * map per document, since uniqueness is only required within a document.
 */
export function d2Salt(source: string, seen: Map<string, number>): string {
  const digest = fnv1a(source);
  const nth = (seen.get(digest) ?? 0) + 1;
  seen.set(digest, nth);
  return nth === 1 ? `d2-${digest}` : `d2-${digest}-${nth}`;
}

// ── RENDER ───────────────────────────────────────────────────────

/**
 * Compile + render one d2 diagram to an SVG string.
 *
 * The reader reaches this only for figures nobody drew ahead of time — a fence newer than its
 * content repo's last CI run, a repo with no workflow, a slideshow's later slides, the authoring
 * preview. Rejects on a malformed diagram so the caller can show a visible `.diagram-error` card,
 * never a blank figure.
 *
 * `compile` and `render` run as ONE queued unit: they are two round-trips to a worker that
 * serves one request at a time, and interleaving another diagram between them is exactly the
 * overlap that hangs it.
 */
export async function renderD2Source(source: string, salt: string): Promise<string> {
  const engine = await d2();
  return enqueue(async () => {
    // `index.d.ts` declares this second argument as `Omit<CompileRequest, "fs">` — an object
    // WRAPPING `options` — but the wrapper assigns whatever it receives straight to `options`.
    // The runtime contract is CompileOptions, and passing the declared shape would nest one
    // level too deep and silently lose the layout. Verified against the engine: `layout` does
    // change geometry, and an unrecognised engine name is rejected outright.
    // BOUND, not detached: `compile` reaches the instance's worker through `this`.
    const compile = engine.compile.bind(engine) as unknown as (
      source: string,
      options: { layout: typeof LAYOUT },
    ) => Promise<{ diagram: Parameters<D2Instance["render"]>[0] }>;
    const result = await compile(source, { layout: LAYOUT });
    return engine.render(result.diagram, d2RenderOptions(salt));
  });
}
