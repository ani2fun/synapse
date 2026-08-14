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
import {
  type BoardManifest,
  boardsDirName,
  decodeManifest,
} from "../islands/diagram/boards";
import * as log from "../log";

/** A lookup that hangs must not hold a lesson open; the file is on the same host. */
const FETCH_BUDGET_MS = 2_000;
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

// ── WALKTHROUGHS ─────────────────────────────────────────────────
// A `d2 boards` fence's figures live BESIDE the lesson, in `_d2/<fence>/`, rather than in the
// content-addressed pool — so the lookup needs the lesson's identity, which the pool's does not.
// Only the ROOT board is fetched here: the others are a click away and the reader may never take
// it, and inlining them all would put bytes nobody reads on every page load.

/** Where a lesson's walkthrough sidecars are served from. */
const boardUrl = (lessonPath: string, fence: string, file: string): string =>
  `${apiBase()}/api/synapse/d2/${encodeURIComponent(fence)}/${encodeURIComponent(file)}` +
  `?lesson=${encodeURIComponent(lessonPath)}`;

/** What one walkthrough needs to render at first paint. */
export interface BoardSet {
  fence: string;
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

/** One file from a walkthrough's sidecar, or null on any failure — a 404 here is the ordinary
 *  state of a repo whose CI has not drawn its figures yet. */
async function fetchBoardFile(lessonPath: string, fence: string, file: string): Promise<string | null> {
  const abort = new AbortController();
  const timer = setTimeout(() => abort.abort(), FETCH_BUDGET_MS);
  try {
    const response = await fetch(boardUrl(lessonPath, fence, file), { signal: abort.signal });
    return response.ok ? await response.text() : null;
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
  renderBoards(source: string, meta: string, lessonPath: string): Promise<BoardSet | null>;
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

    async renderBoards(source: string, meta: string, lessonPath: string): Promise<BoardSet | null> {
      const fence = boardsDirName(source, meta);
      const key = `${lessonPath} ${fence} ${fnv1a(source)}`;
      const cached = rememberBoards(key);
      if (cached != null) {
        drawn += 1;
        return cached;
      }

      const raw = await fetchBoardFile(lessonPath, fence, "boards.json");
      // Everything unreadable is one outcome: not drawn yet. That covers a repo with no CI, a
      // truncated file, and a manifest written by a generator this build does not know — the last
      // of which is why a content repo can upgrade before the app does without breaking a page.
      let manifest: BoardManifest | null = null;
      try {
        manifest = raw == null ? null : decodeManifest(JSON.parse(raw));
      } catch {
        manifest = null;
      }
      // The manifest records the source it was drawn from. A mismatch means the fence has been
      // edited since CI last ran, and serving the drawn boards would show the reader the PREVIOUS
      // diagram — confidently, with no error. Falling back draws what the author actually wrote.
      if (manifest == null || manifest.source !== fnv1a(source)) {
        fellBack += 1;
        return null;
      }

      const root = manifest.boards.find((board) => board.id === manifest.root);
      const rootSvg = root == null ? null : await fetchBoardFile(lessonPath, fence, `${root.slug}.svg`);
      // A manifest whose root board is missing is a half-written directory; the client can still
      // draw the whole thing from source, so fall back rather than paint an empty figure.
      if (rootSvg == null || !rootSvg.trimStart().startsWith("<svg")) {
        fellBack += 1;
        return null;
      }

      const set: BoardSet = { fence, manifest, rootSvg };
      storeBoards(key, set);
      drawn += 1;
      return set;
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
