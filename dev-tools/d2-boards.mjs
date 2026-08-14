// ── THE MULTI-BOARD MODEL — one d2 source with `layers:` → a navigable board graph ─────────
// A ```d2 boards fence compiles to a TREE of boards, and this module is everything both writers
// of that tree agree on: which fences opt in, what a board is called, what file it lands in, and
// which links between boards are real.
//
// It holds no engine and touches no disk — it takes a compiled `Diagram` (or a raw source) and
// returns data. `render-d2.mjs` draws a content repo's boards with it in CI, `d2-interactive.mjs`
// builds a standalone page with it, and `web/src/lib/islands/diagram/boards.ts` carries the
// reader's copy. The app's copy is pinned against this one by `renderD2Script.test.ts`, because a
// slug or salt that disagrees by one character misses every lookup and does it silently.

// ── THE FENCE VOCABULARY ────────────────────────────────────────────────────────────────────
// Bare marker + quoted options, matching the house forms (```lang run, ```viz widget=x). Case
// sensitive like its siblings: ```D2 boards opts in, ```d2 BOARDS does not.

const BOARDS_META = /(?:^|\s)boards(?:$|\s)/;
const NAME_META = /(?:^|\s)name=(?:"([^"]*)"|(\S+))/;
const ROOT_META = /(?:^|\s)root=(?:"([^"]*)"|(\S+))/;

/** The version stamped into every manifest. A bump redraws every board set on the next CI run. */
export const GENERATOR_VERSION = 1;
/** The sidecar directory beside a lesson — `_`-prefixed, so the catalog walker skips it. */
export const BOARDS_DIR = "_d2";
/** The manifest inside one fence's directory. */
export const MANIFEST_FILE = "boards.json";
/** The board every walkthrough opens on. */
export const ROOT_ID = "root";

/** d2's three board kinds. Only `layers` is navigable here; the other two are sequences. */
const BOARD_KINDS = ["layers", "steps", "scenarios"];
const BOARD_KEYS = new Set(BOARD_KINDS);

/** Whether a fence's info string opts into the multi-board viewer. */
export function isBoardsFence(meta) {
  return BOARDS_META.test(meta ?? "");
}

/** `name="url-shortener"` — the sidecar directory. Null when unset; the caller supplies a hash. */
export function fenceName(meta) {
  const m = NAME_META.exec(meta ?? "");
  return m ? (m[1] ?? m[2]) : null;
}

/** `root="System Context"` — the root board's title, which has no key to derive one from. */
export function rootTitleOf(meta) {
  const m = ROOT_META.exec(meta ?? "");
  return m ? (m[1] ?? m[2]) : null;
}

// ── NAMING ──────────────────────────────────────────────────────────────────────────────────

/**
 * A board id's filename stem: `root.layers.container` → `container`.
 *
 * The kind segments carry no information a reader needs, so they are dropped and the remaining
 * keys joined — a nested board reads as `checkout-payment` rather than
 * `root-layers-checkout-layers-payment`. Two boards can therefore collide; `assignSlugs` is
 * where that is resolved, so this stays a pure function of one id.
 */
export function boardSlug(id) {
  const parts = String(id)
    .split(".")
    .filter((part) => part !== "" && part !== ROOT_ID && !BOARD_KEYS.has(part));
  const joined = parts.length === 0 ? ROOT_ID : parts.join("-");
  const clean = joined
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  // Everything downstream treats a slug as a path segment — the server validates it with
  // `slug_like` before joining it to a lesson directory — so an id made entirely of punctuation
  // has to become a name rather than an empty string or a traversal.
  return clean === "" ? "board" : clean;
}

/** The id suffix one board's SVG carries, unique per board so two boards on a page cannot
 *  collide on `<defs>` ids — a collision loses arrowheads and clips with no error anywhere. */
export function saltForBoard(sourceHash, id) {
  return `d2-${sourceHash}-${boardSlug(id)}`;
}

/** `redirect_handler` → `Redirect Handler`. A layer's key is the only title d2 offers. */
function titleCase(key) {
  return String(key)
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

/** Slugs in board order, disambiguated by suffix so the set is injective. */
function assignSlugs(boards) {
  const taken = new Map();
  for (const board of boards) {
    const base = boardSlug(board.id);
    const nth = (taken.get(base) ?? 0) + 1;
    taken.set(base, nth);
    board.slug = nth === 1 ? base : `${base}-${nth}`;
  }
  return boards;
}

// ── THE BOARD WALK ──────────────────────────────────────────────────────────────────────────

/** Every board link a board's shapes and connections carry. The compiler normalises these to
 *  absolute ids (`root.layers.x`), so they need no resolution — anything else is an external
 *  URL and belongs to the browser. */
function linksOf(board) {
  const out = [];
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
export function boardsOf(diagram, rootTitle) {
  const boards = [];

  const walk = (node, id, title, parent) => {
    let nearest = parent;
    if (!node.isFolderOnly) {
      // `node` rides along so a caller can render this board without walking the tree again;
      // `manifestFor` picks its fields explicitly, so it never reaches the committed JSON.
      boards.push({ id, title, parent, links: linksOf(node), node });
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
  return assignSlugs(boards);
}

/** The board kinds a diagram uses beyond `layers` — sequences the viewer shows but does not
 *  treat as a navigable drill-down. */
export function nonLayerKinds(diagram) {
  const kinds = new Set();
  const walk = (node) => {
    for (const kind of BOARD_KINDS) {
      const children = node[kind] ?? [];
      if (kind !== "layers" && children.length > 0) kinds.add(kind);
      for (const child of children) if (child != null) walk(child);
    }
  };
  walk(diagram);
  return [...kinds];
}

// ── SOURCE-LEVEL LINK AUDIT ─────────────────────────────────────────────────────────────────
// d2 resolves `link:` against the board it is written in and DROPS it when nothing matches —
// no anchor in the SVG, no `link` on the compiled shape, and `d2 validate` still reports
// success. `link: layers.x` written inside `layers.container` is the trap: it looks absolute
// and means `root.layers.container.layers.x`. The compiled tree keeps no trace of the loss, so
// catching it means reading the source.

/** One board-link occurrence, whether or not it survived compilation. */
function scanLinks(source) {
  const found = [];
  const stack = []; // { key, kind } — kind set only when the frame is a board
  let lastKey = null;
  let line = 1;
  let i = 0;
  const n = source.length;

  const boardId = () => {
    let id = ROOT_ID;
    for (const frame of stack) if (frame.kind) id += `.${frame.kind}.${frame.key}`;
    return id;
  };

  const advance = (to) => {
    for (let k = i; k < to && k < n; k += 1) if (source[k] === "\n") line += 1;
    i = to;
  };

  while (i < n) {
    const ch = source[i];

    if (ch === "\n") {
      line += 1;
      i += 1;
      lastKey = null;
      continue;
    }
    if (ch === "#") {
      while (i < n && source[i] !== "\n") i += 1; // comment to end of line
      continue;
    }
    // A block string carries prose, not structure — a `link:` inside one is text.
    if (ch === "|") {
      const fence = source.startsWith("|||", i) ? "|||" : "|";
      const body = i + fence.length;
      const close = source.indexOf(fence, body);
      advance(close === -1 ? n : close + fence.length);
      continue;
    }
    if (ch === "{") {
      const parent = stack[stack.length - 1];
      const kind = parent && BOARD_KEYS.has(parent.key) ? parent.key : null;
      stack.push({ key: lastKey ?? "", kind });
      lastKey = null;
      i += 1;
      continue;
    }
    if (ch === "}") {
      stack.pop();
      lastKey = null;
      i += 1;
      continue;
    }
    if (ch === '"' || ch === "'") {
      const quote = ch;
      let j = i + 1;
      while (j < n && source[j] !== quote) j += quote === '"' && source[j] === "\\" ? 2 : 1;
      const token = source.slice(i + 1, j);
      advance(Math.min(j + 1, n));
      lastKey = token;
      continue;
    }
    // A bare token, and the `:` that may follow it, is the only thing left that matters.
    const token = /^[^\s{}:;#|"']+/.exec(source.slice(i))?.[0];
    if (token == null) {
      if (ch === ":" && lastKey === "link") {
        // Everything to the end of the line is the link's value.
        let end = source.indexOf("\n", i);
        if (end === -1) end = n;
        const value = source.slice(i + 1, end).replace(/[;{}]+$/, "").trim();
        if (value !== "") found.push({ value, board: boardId(), line });
        advance(end);
        lastKey = null;
        continue;
      }
      i += 1;
      continue;
    }
    i += token.length;
    lastKey = token;
  }
  return found;
}

/** A value written on a `link:` inside `from`, resolved to an absolute board id. Null when it
 *  addresses something outside the board tree (an http URL, a mail link). */
export function resolveBoardLink(value, from) {
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

/**
 * Every board link the source writes that the compiler could not honour.
 *
 * `known` is the set of ids that actually rendered. A miss is reported rather than thrown: this
 * runs in six content repos off one checked-out script, and failing them all on a parser
 * disagreement costs more than a warning nobody reads.
 */
export function auditBoardLinks(source, known) {
  const broken = [];
  for (const { value, board, line } of scanLinks(source)) {
    const target = resolveBoardLink(value, board);
    if (target == null || known.has(target)) continue;
    // The overwhelmingly common slip is a sibling addressed as if from the root.
    const sibling = resolveBoardLink(`_.${value}`, board);
    const hint = sibling != null && known.has(sibling) ? `_.${value}` : null;
    broken.push({ value, board, line, hint });
  }
  return broken;
}

// ── THE MANIFEST ────────────────────────────────────────────────────────────────────────────

/**
 * One fence's `boards.json`.
 *
 * `generator` and `source` together are the staleness key: a redraw is triggered by an edited
 * diagram OR by a changed generator, so a rule change here cannot go unnoticed in repos whose
 * figures are already committed.
 */
export function manifestFor({ sourceHash, boards, warnings = [] }) {
  return {
    generator: GENERATOR_VERSION,
    source: sourceHash,
    root: ROOT_ID,
    boards: boards.map(({ id, slug, title, parent, links }) => ({
      id,
      slug,
      title,
      parent: parent ?? null,
      links,
    })),
    warnings,
  };
}
