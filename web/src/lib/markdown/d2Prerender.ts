// ──────────────────────────────────────────────────────────────────
// D2 PRE-RENDER — the SSR half of the ```d2 pipeline
// ──────────────────────────────────────────────────────────────────
// Compiles a lesson's d2 fences to SVG during SSR so the diagrams arrive IN THE HTML, painted
// before any JS runs and without the reader fetching a multi-MB WASM bundle.
//
// This module is reached only through a dynamic import guarded by `import.meta.env.SSR`, which
// is what keeps `@terrastruct/d2` out of the client bundle: `renderLesson` also runs in the
// browser (the authoring preview), and a static edge to here would drag the whole engine into it.
//
// EVERY failure path returns null and lets the caller emit the ordinary source placeholder, so
// the worst case is the client-rendered behaviour rather than a broken page. That safety net is
// also a blindfold — a page whose pre-render silently never runs looks exactly like a page that
// works — so the e2e asserts the SVG is present in the response body, and the Dockerfile asserts
// the pruned image can still load the engine at all.

import { renderD2Source, warmD2 } from "../islands/diagram/d2";
import * as log from "../log";

/** A whole document's pre-rendering gives up after this, and the rest falls back. */
const PAGE_BUDGET_MS = 10_000;
/** …and no single diagram may eat the whole page budget. */
const DIAGRAM_BUDGET_MS = 5_000;
/** Bounded so a large catalog cannot grow the SSR process without limit. */
const CACHE_MAX = 512;

// ── CACHE ────────────────────────────────────────────────────────
// Keyed by salt, which is a fingerprint of the source (`d2Salt`), so an entry is valid for as
// long as the source is unchanged — a content edit mints a different key and needs no
// invalidation. Insertion order is the LRU: a hit re-inserts, an overflow drops the oldest.

const cache = new Map<string, string>();

function remember(salt: string): string | undefined {
  const svg = cache.get(salt);
  if (svg === undefined) return undefined;
  cache.delete(salt);
  cache.set(salt, svg);
  return svg;
}

function store(salt: string, svg: string): void {
  cache.set(salt, svg);
  if (cache.size > CACHE_MAX) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
}

// ── ENABLEMENT ───────────────────────────────────────────────────

/**
 * Whether this process should pre-render at all.
 *
 * `SYNAPSE_D2_PRERENDER=off` is the ops kill switch — it restores the client-rendered behaviour
 * without a deploy. Under vitest it is off by DEFAULT instead: booting a 21 MB WASM worker would
 * make every markdown test slow and non-hermetic, and the one test that does cover this path
 * turns it back on with `=on`.
 */
export function prerenderEnabled(): boolean {
  if (typeof process === "undefined") return false;
  const flag = (process.env.SYNAPSE_D2_PRERENDER ?? "").toLowerCase();
  if (flag === "off") return false;
  if (flag === "on") return true;
  return process.env.VITEST == null;
}

if (prerenderEnabled()) {
  // Boot the worker now rather than on the first lesson request, which would otherwise wear
  // worker start plus WASM instantiation on top of its own compiles.
  warmD2();
  log.debug("d2 pre-render: enabled, warming the engine");
}

// ── SESSION ──────────────────────────────────────────────────────

interface Attempt {
  svg: string | null;
  /** Why it failed, for the log — a fallback that does not say why is half a guard. */
  reason?: string;
}

/** Runs one diagram, or gives up so the caller falls back to a placeholder. */
async function attempt(source: string, salt: string, budgetMs: number): Promise<Attempt> {
  const work = renderD2Source(source, salt);
  // Cache on arrival even if this request has already stopped waiting: the compile is running
  // regardless, and the next request for the same lesson should not repeat it.
  void work.then((svg) => store(salt, svg)).catch(() => undefined);

  let timer: ReturnType<typeof setTimeout> | undefined;
  const expiry = new Promise<Attempt>((resolve) => {
    timer = setTimeout(() => resolve({ svg: null, reason: `no answer in ${budgetMs} ms` }), budgetMs);
  });
  const attempted = work
    .then((svg): Attempt => ({ svg }))
    .catch((error: unknown): Attempt => ({
      svg: null,
      reason: error instanceof Error ? error.message : String(error),
    }));
  try {
    return await Promise.race([attempted, expiry]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

export interface D2Session {
  /** The diagram's SVG, or null to fall back to a client-rendered placeholder. */
  render(source: string, salt: string): Promise<string | null>;
  /** One line per document saying how much of it the server actually drew. */
  done(): void;
}

/**
 * Open one document's pre-render session.
 *
 * The budget is per-DOCUMENT, not per-diagram: diagrams compile one at a time through a single
 * worker, so a page of them must share a deadline or a slow one multiplies. Once the deadline
 * passes, the remaining blocks fall back immediately instead of queueing behind work nobody is
 * waiting for any more.
 */
export function openD2Session(): D2Session {
  const deadline = Date.now() + PAGE_BUDGET_MS;
  let rendered = 0;
  let fellBack = 0;

  return {
    async render(source: string, salt: string): Promise<string | null> {
      const hit = remember(salt);
      if (hit !== undefined) {
        rendered += 1;
        return hit;
      }
      const left = deadline - Date.now();
      if (left <= 0) {
        fellBack += 1;
        if (fellBack === 1) log.warn("d2 pre-render: page budget spent, remaining diagrams fall back");
        return null;
      }
      const { svg, reason } = await attempt(source, salt, Math.min(DIAGRAM_BUDGET_MS, left));
      if (svg == null) {
        fellBack += 1;
        log.warn(`d2 pre-render: ${salt} fell back to the client — ${reason ?? "unknown"}`);
      } else {
        rendered += 1;
      }
      return svg;
    },

    done(): void {
      const total = rendered + fellBack;
      if (total === 0) return;
      const line = `d2 pre-render: ${rendered}/${total} diagram(s) drawn server-side`;
      // A page that falls back entirely still renders — which is why it has to be loud here.
      if (fellBack > 0) log.warn(`${line}, ${fellBack} fell back`);
      else log.debug(line);
    },
  };
}
