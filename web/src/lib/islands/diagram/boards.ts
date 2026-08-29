// ──────────────────────────────────────────────────────────────────
// MULTI-BOARD D2 — the board graph, the walk through it, and the URL
// ──────────────────────────────────────────────────────────────────
// A ```d2 boards fence is one source that compiles to a TREE of boards, and a reader drives it by
// clicking the nodes that carry `link:`. This module is the whole model behind that: what a board
// is called, which file holds it, which board a click lands on, and how the walk is remembered.
//
// It is deliberately free of DOM, Preact and the d2 engine — the reason `D2Boards.tsx` is thin
// and this is what the unit tests exercise (vitest collects `src/**/*.test.ts`, and islands are
// e2e territory).
//
// The first half is a CONTRACT with `dev-tools/d2-boards.mjs`, which draws a content repo's
// boards in CI. The script cannot import this file — it runs from a plain Node checkout in a
// content repo — so it carries its own copy, and `renderD2Script.test.ts` asserts the two agree.
// A slug that disagrees by one character misses every lookup, silently.

import { fnv1a } from "../../hash";

// ── THE FENCE VOCABULARY ─────────────────────────────────────────
// Bare marker + quoted options, matching the house forms (```lang run, ```viz widget=x). Case
// sensitive like its siblings: ```D2 boards opts in, ```d2 BOARDS does not.

const BOARDS_META = /(?:^|\s)boards(?:$|\s)/;
const NAME_META = /(?:^|\s)name=(?:"([^"]*)"|(\S+))/;
const ROOT_META = /(?:^|\s)root=(?:"([^"]*)"|(\S+))/;

/** The manifest version this build understands. An older or newer one is treated as absent, so a
 *  content repo that upgrades before the app does degrades to client rendering rather than
 *  rendering something this code cannot read. */
export const GENERATOR_VERSION = 1;
/** The sidecar directory beside a lesson — `_`-prefixed, so the catalog walker skips it. */
export const BOARDS_DIR = "_d2";
/** The manifest inside one fence's directory. */
export const MANIFEST_FILE = "boards.json";
/** The board every walkthrough opens on. */
export const ROOT_ID = "root";

const BOARD_KINDS = ["layers", "steps", "scenarios"] as const;
const BOARD_KEYS = new Set<string>(BOARD_KINDS);

/** Whether a fence's info string opts into the multi-board viewer. */
export function isBoardsFence(meta: string | null | undefined): boolean {
  return BOARDS_META.test(meta ?? "");
}

/** `name="url-shortener"` — the sidecar directory. Null when unset; the caller supplies a hash. */
export function fenceName(meta: string | null | undefined): string | null {
  const m = NAME_META.exec(meta ?? "");
  return m ? (m[1] ?? m[2] ?? null) : null;
}

/** `root="System Context"` — the root board's title, which has no key to derive one from. */
export function rootTitleOf(meta: string | null | undefined): string | null {
  const m = ROOT_META.exec(meta ?? "");
  return m ? (m[1] ?? m[2] ?? null) : null;
}

/** The directory one fence's boards live in, under the lesson's `_d2/`. */
export function boardsDirName(source: string, meta: string | null | undefined): string {
  return fenceName(meta) ?? fnv1a(source);
}

// ── NAMING ───────────────────────────────────────────────────────

/**
 * A board id's filename stem: `root.layers.container` → `container`.
 *
 * The kind segments carry no information a reader needs, so they are dropped and the remaining
 * keys joined. Two boards can therefore collide; the generator disambiguates with a suffix when
 * it assigns slugs, so this stays a pure function of one id and the manifest is the truth about
 * which slug a board actually got.
 */
export function boardSlug(id: string): string {
  const parts = String(id)
    .split(".")
    .filter((part) => part !== "" && part !== ROOT_ID && !BOARD_KEYS.has(part));
  const joined = parts.length === 0 ? ROOT_ID : parts.join("-");
  const clean = joined
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  // Everything downstream treats a slug as a path segment — the server validates it before
  // joining it to a lesson directory — so an id made entirely of punctuation has to become a
  // name rather than an empty string or a traversal.
  return clean === "" ? "board" : clean;
}

/** The id suffix one board's SVG carries, unique per board so two boards on a page cannot collide
 *  on `<defs>` ids — a collision loses arrowheads and clips with no error anywhere. */
export function saltForBoard(sourceHash: string, id: string): string {
  return `d2-${sourceHash}-${boardSlug(id)}`;
}

/** `redirect_handler` → `Redirect Handler`. A layer's key is the only title d2 offers. */
function titleCase(key: string): string {
  return String(key)
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

// ── THE BOARD WALK ───────────────────────────────────────────────
// Structurally typed rather than imported from `@terrastruct/d2`, so this module stays free of
// the engine and its types. The client fallback (`renderD2Boards`) passes a real `Diagram`.

/** As much of a compiled d2 board as the walk reads. */
export interface CompiledBoard {
  name: string;
  isFolderOnly?: boolean;
  shapes?: { link?: string }[];
  connections?: { link?: string }[];
  layers?: (CompiledBoard | undefined)[];
  steps?: (CompiledBoard | undefined)[];
  scenarios?: (CompiledBoard | undefined)[];
}

/** One board as the viewer knows it. */
export interface BoardMeta {
  id: string;
  slug: string;
  title: string;
  parent: string | null;
  links: string[];
}

/** A walked board, carrying the compiled node so a caller can render it without walking again. */
export interface WalkedBoard extends BoardMeta {
  node: CompiledBoard;
}

function linksOf(board: CompiledBoard): string[] {
  const out: string[] = [];
  for (const item of [...(board.shapes ?? []), ...(board.connections ?? [])]) {
    const link = item?.link;
    if (typeof link !== "string" || link === "") continue;
    if (link !== ROOT_ID && !link.startsWith(`${ROOT_ID}.`)) continue; // external
    if (!out.includes(link)) out.push(link);
  }
  return out;
}

/**
 * The compiled diagram's boards, depth first, root first.
 *
 * A `isFolderOnly` board organises the tree without rendering anything, so it is skipped while
 * its children are still walked — and the children point past it to the nearest ancestor that
 * does render, which is what keeps a breadcrumb meaningful.
 */
export function boardsOf(diagram: CompiledBoard, rootTitle?: string | null): WalkedBoard[] {
  const boards: WalkedBoard[] = [];

  const walk = (node: CompiledBoard, id: string, title: string, parent: string | null): void => {
    let nearest = parent;
    if (node.isFolderOnly !== true) {
      boards.push({ id, slug: "", title, parent, links: linksOf(node), node });
      nearest = id;
    }
    for (const kind of BOARD_KINDS) {
      for (const child of node[kind] ?? []) {
        if (child == null) continue;
        walk(child, `${id}.${kind}.${child.name}`, titleCase(child.name), nearest);
      }
    }
  };

  walk(diagram, ROOT_ID, rootTitle ?? "Overview", null);

  const taken = new Map<string, number>();
  for (const board of boards) {
    const base = boardSlug(board.id);
    const nth = (taken.get(base) ?? 0) + 1;
    taken.set(base, nth);
    board.slug = nth === 1 ? base : `${base}-${nth}`;
  }
  return boards;
}

// ── THE MANIFEST ─────────────────────────────────────────────────

/** A `link:` the author wrote that d2 dropped — see `auditBoardLinks`. */
export interface BoardWarning {
  value: string;
  board: string;
  line: number;
  hint: string | null;
}

/** One fence's `boards.json`, as committed beside its lesson. */
export interface BoardManifest {
  generator: number;
  source: string;
  root: string;
  boards: BoardMeta[];
  warnings: BoardWarning[];
}

function isBoardMeta(value: unknown): value is BoardMeta {
  if (typeof value !== "object" || value === null) return false;
  const board = value as Record<string, unknown>;
  return (
    typeof board.id === "string" &&
    typeof board.slug === "string" &&
    typeof board.title === "string" &&
    (board.parent === null || typeof board.parent === "string") &&
    Array.isArray(board.links) &&
    board.links.every((link) => typeof link === "string")
  );
}

/**
 * A manifest read off the wire, or null.
 *
 * Null covers every way this can go wrong at once — a media route that answered with someone
 * else's markup, a truncated file, a repo drawn by a generator this build does not know. The
 * caller treats all of them as "not drawn yet" and falls back to the client renderer, which is
 * the same floor every other d2 miss lands on.
 */
export function decodeManifest(raw: unknown): BoardManifest | null {
  if (typeof raw !== "object" || raw === null) return null;
  const value = raw as Record<string, unknown>;
  if (value.generator !== GENERATOR_VERSION) return null;
  if (typeof value.source !== "string" || typeof value.root !== "string") return null;
  if (!Array.isArray(value.boards) || value.boards.length === 0) return null;
  if (!value.boards.every(isBoardMeta)) return null;
  const boards = value.boards as BoardMeta[];
  if (!boards.some((board) => board.id === value.root)) return null;
  const warnings = Array.isArray(value.warnings) ? (value.warnings as BoardWarning[]) : [];
  return { generator: value.generator, source: value.source, root: value.root, boards, warnings };
}

// ── THE INDEX ────────────────────────────────────────────────────

/** The manifest turned into the lookups the viewer needs on every click. */
export interface BoardIndex {
  root: string;
  order: BoardMeta[];
  get(id: string): BoardMeta | undefined;
  /** A board by the slug a URL carries. */
  bySlug(slug: string): BoardMeta | undefined;
  /** Root → … → `id`, for the breadcrumb. Empty when `id` is unknown. */
  trail(id: string): BoardMeta[];
}

export function indexBoards(manifest: BoardManifest): BoardIndex {
  const byId = new Map(manifest.boards.map((board) => [board.id, board]));
  const bySlug = new Map(manifest.boards.map((board) => [board.slug, board]));
  return {
    root: manifest.root,
    order: manifest.boards,
    get: (id) => byId.get(id),
    bySlug: (slug) => bySlug.get(slug),
    trail(id) {
      const out: BoardMeta[] = [];
      // `parent` is written by the generator, so a cycle would take a corrupt manifest — but the
      // walk is bounded by the board count anyway rather than trusting that.
      const seen = new Set<string>();
      let at = byId.get(id);
      while (at != null && !seen.has(at.id)) {
        seen.add(at.id);
        out.unshift(at);
        at = at.parent == null ? undefined : byId.get(at.parent);
      }
      return out;
    },
  };
}

/**
 * Whether this diagram is something a reader can move around IN.
 *
 * A one-board tree is a picture, not a walkthrough: back, forward and home are all disabled and
 * the menu offers the board already on screen. Both chrome skins ask this and render nothing when
 * the answer is no, so a plain figure carries no controls describing their own uselessness.
 *
 * It counts boards rather than reading the fence's kind, because a `boards` fence that declares
 * one layer is just as empty as a plain one.
 */
export function canNavigate(index: BoardIndex): boolean {
  return index.order.length > 1;
}

// ── THE WALK THROUGH IT ──────────────────────────────────────────
// Board history is the component's own, never the browser's: a reader who drilled four levels
// down should still leave the lesson with one press of Back. The URL still carries the board, so
// a link is shareable — see `boardSearch`.

export interface BoardHistory {
  entries: string[];
  at: number;
}

export const startHistory = (id: string): BoardHistory => ({ entries: [id], at: 0 });

export const currentBoard = (history: BoardHistory): string => history.entries[history.at]!;
export const canGoBack = (history: BoardHistory): boolean => history.at > 0;
export const canGoForward = (history: BoardHistory): boolean =>
  history.at < history.entries.length - 1;

/** Navigate to `id`. Re-entering the board already shown is a no-op, so a double click does not
 *  fill the trail with duplicates; anything else truncates the forward tail, like a browser. */
export function pushBoard(history: BoardHistory, id: string): BoardHistory {
  if (currentBoard(history) === id) return history;
  const entries = [...history.entries.slice(0, history.at + 1), id];
  return { entries, at: entries.length - 1 };
}

export const goBack = (history: BoardHistory): BoardHistory =>
  canGoBack(history) ? { ...history, at: history.at - 1 } : history;

export const goForward = (history: BoardHistory): BoardHistory =>
  canGoForward(history) ? { ...history, at: history.at + 1 } : history;

/** Home is a navigation, not a rewind: it keeps the trail so Back still returns where you were. */
export const goHome = (history: BoardHistory, root: string): BoardHistory =>
  pushBoard(history, root);

/**
 * The board one STEP away in walk order, or null at either end.
 *
 * History alone leaves the transport dead on arrival: a freshly loaded page has been nowhere, so
 * back and forward are both correctly disabled and the only way deeper is the node in the figure
 * or the menu. Readers read that as broken controls. Stepping is the fallback — when history has
 * nothing to offer, the arrows walk the boards in the order the manifest lists them, which for a
 * C4 stack is exactly context → container → code.
 *
 * History still wins where it exists, so a reader who jumped somewhere with the menu gets back to
 * where they came from rather than to whatever happens to sit beside it.
 */
export function stepBoard(index: BoardIndex, at: string, delta: 1 | -1): string | null {
  const i = index.order.findIndex((board) => board.id === at);
  if (i === -1) return null;
  return index.order[i + delta]?.id ?? null;
}

/** Whether the transport can move at all in that direction — history first, then a step. */
export const canStepBack = (index: BoardIndex, history: BoardHistory): boolean =>
  canGoBack(history) || stepBoard(index, currentBoard(history), -1) != null;

export const canStepForward = (index: BoardIndex, history: BoardHistory): boolean =>
  canGoForward(history) || stepBoard(index, currentBoard(history), 1) != null;

/** Back, then a step backwards when there is no history to spend. */
export function walkBack(index: BoardIndex, history: BoardHistory): BoardHistory {
  if (canGoBack(history)) return goBack(history);
  const prev = stepBoard(index, currentBoard(history), -1);
  return prev == null ? history : pushBoard(history, prev);
}

export function walkForward(index: BoardIndex, history: BoardHistory): BoardHistory {
  if (canGoForward(history)) return goForward(history);
  const next = stepBoard(index, currentBoard(history), 1);
  return next == null ? history : pushBoard(history, next);
}

// ── THE URL ──────────────────────────────────────────────────────
// A query parameter, not the fragment: `rehypeSlug` gives every heading an id, so the fragment
// belongs to the prose. A reader can deep-link a heading and a board at the same time this way.

export const BOARD_PARAM = "board";

/** The board a URL asks for, or null. Unknown slugs are ignored rather than erroring — a stale
 *  link should open the diagram at its root, not break the page. */
export function boardFromSearch(search: string, index: BoardIndex): string | null {
  const slug = new URLSearchParams(search).get(BOARD_PARAM);
  if (slug == null) return null;
  return index.bySlug(slug)?.id ?? null;
}

/** The search string for a URL pointing at `id`. The root board drops the parameter, so the
 *  canonical address of an unopened diagram is the bare lesson URL. */
export function boardSearch(search: string, index: BoardIndex, id: string): string {
  const params = new URLSearchParams(search);
  const board = index.get(id);
  if (board == null || id === index.root) params.delete(BOARD_PARAM);
  else params.set(BOARD_PARAM, board.slug);
  const query = params.toString();
  return query === "" ? "" : `?${query}`;
}

// ── LINK RESOLUTION ──────────────────────────────────────────────
// The reader does not need this — the compiler writes absolute ids into every anchor it emits,
// so a click is a map lookup. The `/d2` editor does: it shows an author the links d2 dropped
// while they are still typing, and that means resolving the source's raw values.

/** A value written on a `link:` inside `from`, resolved to an absolute board id. Null when it
 *  addresses something outside the board tree (an http URL, a mail link). */
export function resolveBoardLink(value: string, from: string): string | null {
  const raw = String(value).trim().replace(/^["']|["']$/g, "");
  if (raw === "" || /^[a-z][a-z0-9+.-]*:/i.test(raw)) return null; // any URL scheme
  if (raw === ROOT_ID || raw.startsWith(`${ROOT_ID}.`)) return raw;

  // `_` steps to the parent BOARD, so each leading `_.` drops one kind+key pair.
  let rest = raw;
  let base = from;
  while (rest === "_" || rest.startsWith("_.")) {
    const cut = base.lastIndexOf(".", base.lastIndexOf(".") - 1);
    base = cut <= 0 ? ROOT_ID : base.slice(0, cut);
    if (rest === "_") return base;
    rest = rest.slice(2);
  }
  return rest === "" ? base : `${base}.${rest}`;
}
