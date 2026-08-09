// ──────────────────────────────────────────────────────────────────
// D2 FIGURES, DRAWN AHEAD OF TIME — the SSR half of the ```d2 pipeline
// ──────────────────────────────────────────────────────────────────
// A content repo compiles its ```d2 fences in CI (`dev-tools/render-d2.mjs`) and commits the SVG
// to `_media/d2/<hash>.svg`. This looks that file up while rendering a lesson and inlines it, so
// the diagram arrives with the page and no engine is downloaded or run to put it there.
//
// It FETCHES, it does not compile. Compiling here is what the pod cannot afford: d2's Go/wasm
// engine peaks at ~5.2 GB of RSS on one 23-diagram lesson, against a 256Mi container. A CI runner
// has that memory and does the work once per content change instead of once per deploy.
//
// Inline, not `<img src>`: an SVG loaded as an image is inert, and these carry `<a href>` links
// and `<title>` tooltips that authors rely on.
//
// A miss is not an error — a fence added before CI ran, or a satellite repo with no workflow yet,
// simply falls back to the source placeholder and the client draws it. That makes a wholly broken
// lookup indistinguishable from a working page, which is why `d2-prerender.spec.ts` asserts on
// the SVG being in the response body rather than on the page looking right.

import { apiBase } from "../api/client";
import { fnv1a } from "../hash";
import * as log from "../log";

/** A lookup that hangs must not hold a lesson open; the file is on the same host. */
const FETCH_BUDGET_MS = 2_000;
/** Bounded so a large catalog cannot grow the SSR process without limit. */
const CACHE_MAX = 512;

// ── CACHE ────────────────────────────────────────────────────────
// Keyed by the source's hash, which is also the filename, so an entry is valid exactly as long
// as the diagram is unchanged — an edit mints a different key and needs no invalidation.
// Insertion order is the LRU: a hit re-inserts, an overflow drops the oldest.

const cache = new Map<string, string>();

function remember(hash: string): string | undefined {
  const svg = cache.get(hash);
  if (svg === undefined) return undefined;
  cache.delete(hash);
  cache.set(hash, svg);
  return svg;
}

function store(hash: string, svg: string): void {
  cache.set(hash, svg);
  if (cache.size > CACHE_MAX) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
}

// ── ENABLEMENT ───────────────────────────────────────────────────

/**
 * Whether to inline pre-drawn figures at all.
 *
 * `SYNAPSE_D2_PRERENDER=off` is the ops kill switch — it restores the client-rendered behaviour
 * without a deploy. Under vitest it is off by DEFAULT instead, so markdown tests neither reach
 * the network nor depend on a media tree; the tests that do cover this path turn it back on.
 */
export function prerenderEnabled(): boolean {
  if (typeof process === "undefined") return false;
  const flag = (process.env.SYNAPSE_D2_PRERENDER ?? "").toLowerCase();
  if (flag === "off") return false;
  if (flag === "on") return true;
  return process.env.VITEST == null;
}

// ── LOOKUP ───────────────────────────────────────────────────────

/** `_media/d2/<hash>.svg` through the media route, which probes every mounted checkout. */
async function fetchDrawn(hash: string): Promise<string | null> {
  const url = `${apiBase()}/media/d2/${hash}.svg`;
  const abort = new AbortController();
  const timer = setTimeout(() => abort.abort(), FETCH_BUDGET_MS);
  try {
    const response = await fetch(url, { signal: abort.signal });
    if (!response.ok) return null;
    const svg = await response.text();
    // A media route that answers with something other than an SVG (a stray index.html, a
    // rewritten 404 page) must be a miss, not a figure made of someone else's markup.
    return svg.trimStart().startsWith("<svg") ? svg : null;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

export interface D2Session {
  /** The diagram's SVG, or null to fall back to a client-rendered placeholder. */
  render(source: string, salt: string): Promise<string | null>;
  /** One line per document saying how much of it arrived pre-drawn. */
  done(): void;
}

/**
 * Open one document's lookup session.
 *
 * `salt` is the id suffix the reader's copy would have used. The file is drawn with the salt of a
 * diagram's FIRST occurrence, so when a document repeats one verbatim the second copy is
 * re-salted here — cheap on a ~16 KB string, and it keeps element ids unique within a page.
 */
export function openD2Session(): D2Session {
  let drawn = 0;
  let fellBack = 0;

  return {
    async render(source: string, salt: string): Promise<string | null> {
      const hash = fnv1a(source);
      const svg = remember(hash) ?? (await fetchDrawn(hash));
      if (svg == null) {
        fellBack += 1;
        return null;
      }
      store(hash, svg);
      drawn += 1;
      const base = `d2-${hash}`;
      return salt === base ? svg : svg.replaceAll(base, salt);
    },

    done(): void {
      const total = drawn + fellBack;
      if (total === 0) return;
      const line = `d2: ${drawn}/${total} figure(s) inlined from _media/d2`;
      // A page that falls back entirely still renders, which is why this is loud: it means the
      // content repo has not drawn its diagrams, and every reader is paying for the engine.
      if (fellBack > 0) log.warn(`${line}, ${fellBack} not drawn yet — the client will compile them`);
      else log.debug(line);
    },
  };
}
