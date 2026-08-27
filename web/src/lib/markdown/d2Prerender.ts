// ──────────────────────────────────────────────────────────────────
// D2 FIGURES, DRAWN ON DEMAND — the SSR half of the ```d2 pipeline
// ──────────────────────────────────────────────────────────────────
// A ```d2 fence is drawn by the `d2-render` sidecar while the lesson renders, and the SVG is
// inlined — so the diagram arrives with the page and no engine is downloaded or run to put it
// there. The sidecar caches by content, so a given diagram is drawn once, ever.
//
// It ASKS, it does not compile. Compiling in this process is what the pod cannot afford: d2's
// Go/wasm engine peaks at ~5.2 GB of RSS on one 23-diagram lesson, against a 256Mi container. The
// native engine the sidecar runs peaks at ~172 MB, in a container with its own limit, so a render
// spike cannot take the page tier down with it (ADR-RS009).
//
// Inline, not `<img src>`: an SVG loaded as an image is inert, and these carry `<a href>` links
// and `<title>` tooltips that authors rely on.
//
// A miss is not an error — a sidecar that is down, slow, or refuses a malformed diagram falls back
// to the source placeholder and the client draws it. That makes a wholly dead renderer
// indistinguishable from a working page, which is why `d2-prerender.spec.ts` asserts on the SVG
// being in the response body rather than on the page looking right.

import { fnv1a } from "../hash";
import { type BoardManifest, decodeManifest } from "../islands/diagram/boards";
import * as log from "../log";

/** A render that hangs must not hold a lesson open; the sidecar is in the same pod. Generous
 *  enough for a cold diagram (~70 ms warm, and the first one pays the engine's own start-up),
 *  short enough that a wedged sidecar costs a page some latency rather than the request. */
const FETCH_BUDGET_MS = 5_000;
/** Bounded so a large catalog cannot grow the SSR process without limit. */
const CACHE_MAX = 512;
/** A walkthrough's entry is a manifest plus a whole board, several times the size of a lone
 *  figure, so its cache is bounded in BYTES rather than entries — this pod has 256Mi. */
const BOARD_CACHE_MAX_BYTES = 4 * 1024 * 1024;

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
 * Where the render sidecar is, or null to leave every figure to the client.
 *
 * One variable, and it carries both the switch and the address: unset means there is no renderer
 * and the reader compiles, which is exactly the behaviour to fall back to. Emptying it is the ops
 * kill switch, and figment reads an empty env var as `Some("")`, so the blank case is checked.
 *
 * Deliberately NOT `SYNAPSE_D2_PRERENDER`. That name has already meant two different things — up
 * to `0c50378` it meant "compile here", the 5.2 GB path — and RS007 records that a rollback across
 * that line must turn it off first. A third meaning on the same name would make the next rollback
 * a guess.
 */
export function rendererUrl(): string | null {
  if (typeof process === "undefined") return null;
  const url = (process.env.SYNAPSE_D2_RENDER_URL ?? "").trim();
  return url === "" ? null : url.replace(/\/+$/, "");
}

/** Whether to inline server-drawn figures at all. */
export function prerenderEnabled(): boolean {
  return rendererUrl() !== null;
}

// ── THE RENDER REQUEST ───────────────────────────────────────────

/** Ask the sidecar to draw one fence. Null on anything at all going wrong — see the header. */
async function drawFigure(source: string): Promise<string | null> {
  const base = rendererUrl();
  if (base === null) return null;
  const abort = new AbortController();
  const timer = setTimeout(() => abort.abort(), FETCH_BUDGET_MS);
  try {
    const response = await fetch(`${base}/render`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ source }),
      signal: abort.signal,
    });
    if (!response.ok) return null;
    const svg = await response.text();
    // A proxy or error page answering in the sidecar's place must be a miss, not a figure made of
    // someone else's markup.
    return svg.trimStart().startsWith("<svg") ? svg : null;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

// ── WALKTHROUGHS ─────────────────────────────────────────────────
// A `d2 boards` fence compiles to a TREE of boards. The sidecar draws every one of them on the
// first request and returns the graph plus the ROOT — the others are a click away that many
// readers never take, and inlining them all would put bytes nobody reads on every page load. They
// are already cached when that click comes.
//
// Content-addressed like everything else, which is what retired the lesson-relative `_d2/`
// sidecars: a walkthrough is identified by its source, so it no longer matters which lesson it is
// in, and `RenderContext` went with it.

/** What one walkthrough needs to render at first paint. */
export interface BoardSet {
  manifest: BoardManifest;
  rootSvg: string;
}

const boardCache = new Map<string, BoardSet>();
let boardCacheBytes = 0;

const sizeOf = (set: BoardSet): number => set.rootSvg.length + JSON.stringify(set.manifest).length;

function rememberBoards(key: string): BoardSet | undefined {
  const set = boardCache.get(key);
  if (set === undefined) return undefined;
  boardCache.delete(key);
  boardCache.set(key, set);
  return set;
}

function storeBoards(key: string, set: BoardSet): void {
  boardCache.set(key, set);
  boardCacheBytes += sizeOf(set);
  while (boardCacheBytes > BOARD_CACHE_MAX_BYTES) {
    const oldest = boardCache.keys().next().value;
    if (oldest === undefined) break;
    const dropped = boardCache.get(oldest);
    boardCache.delete(oldest);
    boardCacheBytes -= dropped == null ? 0 : sizeOf(dropped);
  }
}

/** Ask the sidecar to draw a whole walkthrough. Null on anything at all going wrong. */
async function drawWalkthrough(source: string, meta: string): Promise<BoardSet | null> {
  const base = rendererUrl();
  if (base === null) return null;
  const abort = new AbortController();
  const timer = setTimeout(() => abort.abort(), FETCH_BUDGET_MS);
  try {
    const response = await fetch(`${base}/boards`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ source, meta }),
      signal: abort.signal,
    });
    if (!response.ok) return null;
    const payload: unknown = await response.json();
    if (typeof payload !== "object" || payload === null) return null;
    const { manifest, rootSvg } = payload as { manifest?: unknown; rootSvg?: unknown };
    // Everything unreadable is one outcome: not drawn. That covers a truncated response, a proxy
    // answering in the sidecar's place, and a manifest from a generator this build does not know —
    // the last of which is why the sidecar can be upgraded before the app without breaking a page.
    const decoded = decodeManifest(manifest);
    if (decoded === null || typeof rootSvg !== "string") return null;
    // The manifest records the source it was built from. A mismatch would mean showing the reader
    // a DIFFERENT diagram, confidently and with no error.
    if (decoded.source !== fnv1a(source)) return null;
    if (!rootSvg.trimStart().startsWith("<svg")) return null;
    return { manifest: decoded, rootSvg };
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

export interface D2Session {
  /** The diagram's SVG, or null to fall back to a client-rendered placeholder. */
  render(source: string, salt: string): Promise<string | null>;
  /** A walkthrough's board graph and its root board, or null to fall back to the client. */
  renderBoards(source: string, meta: string): Promise<BoardSet | null>;
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
      const svg = remember(hash) ?? (await drawFigure(source));
      if (svg == null) {
        fellBack += 1;
        return null;
      }
      store(hash, svg);
      drawn += 1;
      const base = `d2-${hash}`;
      return salt === base ? svg : svg.replaceAll(base, salt);
    },

    async renderBoards(source: string, meta: string): Promise<BoardSet | null> {
      // `meta` is part of the key, not just the request: it carries `root=`, which names the root
      // board, so the same source under a different title is a different walkthrough.
      const key = `${fnv1a(source)}\u0000${meta}`;
      const cached = rememberBoards(key);
      if (cached != null) {
        drawn += 1;
        return cached;
      }
      const set = await drawWalkthrough(source, meta);
      if (set == null) {
        fellBack += 1;
        return null;
      }
      storeBoards(key, set);
      drawn += 1;
      return set;
    },

    done(): void {
      const total = drawn + fellBack;
      if (total === 0) return;
      const line = `d2: ${drawn}/${total} figure(s) inlined from the renderer`;
      // A page that falls back entirely still renders, which is why this is loud: it means the
      // sidecar is unreachable and every reader is paying for the engine instead.
      if (fellBack > 0) log.warn(`${line}, ${fellBack} not drawn — the client will compile them`);
      else log.debug(line);
    },
  };
}
