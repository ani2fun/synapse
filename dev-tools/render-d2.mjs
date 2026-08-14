#!/usr/bin/env node
// ── D2 → SVG, AHEAD OF THE REQUEST ────────────────────────────────────────────
// Walks a content checkout, compiles every ```d2 fence, and writes the SVG a reader's page will
// inline. Two artifact shapes, chosen by the fence:
//
//   ```d2         → `_media/d2/<hash>.svg`, one file in a content-addressed pool
//   ```d2 boards  → `<lesson-dir>/_d2/<name>/`, one SVG per board plus `boards.json`
//
// A walkthrough's figures sit beside the lesson that shows them, the way `_c4-docs/` and
// `<stem>.editorial.md` do, so moving a lesson moves its diagrams and deleting one deletes them.
//
// It lives here, in the app repo, rather than in each content repo, because the hash and the
// render options MUST match `web/src/lib/islands/diagram/d2.ts` exactly — a different salt or a
// different pad is a cache miss, and a cache miss is silent (the page falls back to the client
// renderer and simply gets slow again). `renderD2Script.test.ts` pins that agreement.
//
// Why not on the server: d2's Go/wasm engine peaks at ~5.2 GB of RSS rendering one 23-diagram
// lesson. That is fine on a CI runner and impossible in a 256Mi pod.
//
// Usage:  node dev-tools/render-d2.mjs <content-root> [--prune] [--strict] [--quiet]

import { readdir, readFile, mkdir, writeFile, stat, unlink, rm } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";

import {
  BOARDS_DIR,
  GENERATOR_VERSION,
  MANIFEST_FILE,
  auditBoardLinks,
  boardSlug,
  boardsOf,
  fenceName,
  isBoardsFence,
  manifestFor,
  nonLayerKinds,
  rootTitleOf,
  saltForBoard,
} from "./d2-boards.mjs";

export {
  BOARDS_DIR,
  GENERATOR_VERSION,
  MANIFEST_FILE,
  auditBoardLinks,
  boardSlug,
  boardsOf,
  fenceName,
  isBoardsFence,
  manifestFor,
  resolveBoardLink,
  rootTitleOf,
  saltForBoard,
} from "./d2-boards.mjs";

// ── THE CONTRACT WITH THE RUNTIME ────────────────────────────────────────────
// Both halves of this are asserted against the app's own copies in renderD2Script.test.ts.

/** FNV-1a (32-bit) as 8 hex digits — mirrors `web/src/lib/hash.ts`. */
export function fnv1a(input) {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

/** The layout engine and render options — mirrors `renderD2Source`. */
export const LAYOUT = "elk";
export const renderOptions = (salt) => ({ themeID: 0, pad: 20, noXMLTag: true, salt });
/** The salt a diagram's FIRST occurrence gets — mirrors `d2Salt`. */
export const saltFor = (source) => `d2-${fnv1a(source)}`;

// ── FENCE EXTRACTION ─────────────────────────────────────────────────────────

// (1) the opening run, (2) the info string, (3) the body, then a closing run AT LEAST AS LONG.
//
// Three or more backticks, per CommonMark, and the info string of a backtick fence may not
// contain a backtick. Both halves matter here. A ````d2 fence used to be read as language "" with
// the fourth backtick swallowed into the info string, so it was skipped — the page still rendered
// it, because remark reads it correctly, and it simply never got drawn. And the closing run being
// at least as long is what keeps ````markdown … ```d2 … ```` a documentation example instead of a
// diagram.
const FENCE = /^(`{3,})([^`\n]*)\n([\s\S]*?)^\1`*[ \t]*$/gm;

/**
 * Every ```d2 fence in a markdown document, in order, with its info string.
 *
 * `source` must be byte-for-byte what remark's `code` node carries, because that string is what
 * the reader hashes to find this diagram's file. Two details do the work: the language is
 * trimmed and lowercased (so ```D2 counts), and the body carries NO trailing newline — the one
 * before the closing fence belongs to the fence, not to the source. A stray "\n" here changes
 * every hash and silently misses every lookup. `renderD2Script.test.ts` parses the same markdown
 * with remark and compares.
 *
 * `meta` is everything after the language, matching remark's `code.meta`: the marker that
 * chooses between the two artifact shapes lives there. `line` is the 1-based line the opening
 * fence sits on, so a diagnostic about the diagram can name a line in the DOCUMENT — a line
 * number counted inside the fence points a reader at the wrong place.
 */
export function d2Blocks(markdown) {
  const found = [];
  for (const match of markdown.matchAll(FENCE)) {
    // The info string is one capture; the language is its first word and the meta is the rest,
    // which is how remark splits `code.lang` from `code.meta`.
    const info = (match[2] ?? "").trim();
    const space = info.search(/\s/);
    const lang = (space === -1 ? info : info.slice(0, space)).toLowerCase();
    if (lang !== "d2") continue;
    const before = markdown.slice(0, match.index);
    let line = 1;
    for (let i = 0; i < before.length; i += 1) if (before[i] === "\n") line += 1;
    found.push({
      source: match[3].replace(/\n$/, ""),
      meta: space === -1 ? "" : info.slice(space + 1).trim(),
      line,
    });
  }
  return found;
}

/** Just the sources — the shape the pool is keyed on. */
export function d2Fences(markdown) {
  return d2Blocks(markdown).map((block) => block.source);
}

/**
 * The d2 engine, resolved from the WORKING DIRECTORY rather than from this file.
 *
 * ESM resolves a bare specifier against the importing module's location, and this script is
 * checked out beside the content it draws rather than beside an install. Whoever runs it does the
 * `npm i`, in their own directory, so that is where to look — with a plain import as the fallback
 * for the case where the two happen to coincide.
 */
async function loadEngine() {
  const { createRequire } = await import("node:module");
  const { pathToFileURL } = await import("node:url");
  let resolved;
  try {
    const require = createRequire(join(process.cwd(), "package.json"));
    resolved = require.resolve("@terrastruct/d2");
  } catch {
    // Installed beside the script instead — the plain specifier is then correct.
    return await import("@terrastruct/d2");
  }
  // `require.resolve` picks the package's `require` condition, and that build exports nothing to
  // an `import()` — the namespace comes back empty and `D2` is undefined. Reach the ESM build
  // beside it. If this layout ever changes the import throws here, in CI, rather than producing
  // an engine that silently draws nothing.
  const esm = join(resolved.slice(0, resolved.indexOf(`${sep}dist${sep}`)), "dist", "node-esm", "index.js");
  return await import(pathToFileURL(esm).href);
}

/** ONE engine per run, built on first use — constructing it spawns a worker and instantiates a
 *  ~21 MB wasm module, and a checkout with nothing to draw must not pay for it. */
function engineOnce() {
  let started = null;
  return () => {
    if (started == null) started = loadEngine().then(({ D2 }) => new D2());
    return started;
  };
}

// ── THE WALK ─────────────────────────────────────────────────────────────────

/** Every markdown file under `root`, and every directory visited on the way — the directories
 *  matter because a `_d2/` orphaned by a deleted lesson is only reachable from its parent. */
async function walkContent(dir, out = { files: [], dirs: [] }) {
  out.dirs.push(dir);
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (entry.name.startsWith(".") || entry.name === "node_modules" || entry.name === "_media") {
      continue;
    }
    const path = join(dir, entry.name);
    if (entry.isDirectory()) await walkContent(path, out);
    else if (entry.name.endsWith(".md")) out.files.push(path);
  }
  return out;
}

// ── MAIN ─────────────────────────────────────────────────────────────────────

async function main() {
  const args = process.argv.slice(2);
  const root = resolve(args.find((a) => !a.startsWith("--")) ?? ".");
  const prune = args.includes("--prune");
  const strict = args.includes("--strict");
  const quiet = args.includes("--quiet");
  const say = (line) => {
    if (!quiet) console.log(line);
  };
  const rel = (path) => relative(root, path) || ".";

  const engine = engineOnce();
  let problems = 0;

  // ── COLLECT ────────────────────────────────────────────────────────────────
  // Two piles, because the fence decides where a figure lands. Walkthroughs keep the file they
  // came from; a pooled fence needs only its content.

  const pool = new Map(); // hash → source
  const walkthroughs = []; // { file, dir, name, source, meta }
  const declared = new Map(); // lesson dir → Set of `_d2` names that dir still declares
  let fenceCount = 0;

  /** Say what a walkthrough's dead links are, whether it was just drawn or read back from disk —
   *  a diagnostic only an unchanged diagram never repeats is one nobody ever acts on. */
  const reportBroken = (work, broken) => {
    for (const miss of broken) {
      problems += 1;
      const hint = miss.hint ? ` — did you mean \`link: ${miss.hint}\`?` : "";
      // The audit counts lines inside the diagram; the fence's own line makes that a file line.
      const at = work.line + miss.line;
      console.warn(`${rel(work.file)}:${at}: \`link: ${miss.value}\` in board ${miss.board} names no board${hint}`);
    }
  };

  const { files, dirs } = await walkContent(root);
  for (const file of files) {
    const dir = dirname(file);
    if (!declared.has(dir)) declared.set(dir, new Set());
    const names = declared.get(dir);
    for (const { source, meta, line } of d2Blocks(await readFile(file, "utf8"))) {
      fenceCount += 1;
      if (!isBoardsFence(meta)) {
        pool.set(fnv1a(source), source);
        continue;
      }
      const name = fenceName(meta) ?? fnv1a(source);
      if (names.has(name)) {
        // Two fences writing one directory would race and the loser would vanish, so this is the
        // one condition that stops the run outright rather than warning.
        throw new Error(`${rel(file)}: two \`\`\`d2 boards fences in this directory are both named "${name}"`);
      }
      names.add(name);
      walkthroughs.push({ file, dir, name, source, meta, line });
    }
  }
  say(`${fenceCount} d2 fence(s) across the checkout · ${pool.size} pooled · ${walkthroughs.length} walkthrough(s)`);

  // ── THE POOL: `_media/d2/<hash>.svg` ───────────────────────────────────────

  const outDir = join(root, "_media", "d2");
  if (pool.size > 0) {
    await mkdir(outDir, { recursive: true });
    // Idempotent: an unchanged diagram keeps its file, so a content edit only pays for what it
    // actually changed and the commit stays readable.
    const missing = [];
    for (const [hash, source] of pool) {
      try {
        await stat(join(outDir, `${hash}.svg`));
      } catch {
        missing.push([hash, source]);
      }
    }
    say(`${pool.size - missing.length} already drawn · ${missing.length} to draw`);

    if (missing.length > 0) {
      const d2 = await engine();
      for (const [hash, source] of missing) {
        // One at a time: the client keeps a single in-flight request, so concurrent calls on one
        // instance clobber each other's continuation and hang.
        const compiled = await d2.compile(source, { layout: LAYOUT });
        const svg = await d2.render(compiled.diagram, renderOptions(saltFor(source)));
        await writeFile(join(outDir, `${hash}.svg`), svg, "utf8");
        say(`  drew ${hash} (${source.trim().split("\n")[0].slice(0, 48)})`);
      }
    }

    // Orphans are reported by default and only removed on request: a half-checked-out tree, or a
    // run pointed at the wrong directory, would otherwise delete every diagram in the repo.
    const onDisk = (await readdir(outDir)).filter((f) => f.endsWith(".svg"));
    const orphans = onDisk.filter((f) => !pool.has(f.replace(/\.svg$/, "")));
    if (orphans.length > 0) {
      if (prune) {
        for (const file of orphans) await unlink(join(outDir, file));
        say(`pruned ${orphans.length} orphan(s)`);
      } else {
        say(`${orphans.length} orphan(s) no fence refers to (pass --prune to remove)`);
      }
    }
    say(`${rel(outDir)} holds ${pool.size} diagram(s)`);
  }

  // ── WALKTHROUGHS: `<lesson-dir>/_d2/<name>/` ───────────────────────────────

  for (const work of walkthroughs) {
    const dir = join(work.dir, BOARDS_DIR, work.name);
    const hash = fnv1a(work.source);

    // The staleness key is the source AND the generator, so a change to how boards are named or
    // linked redraws committed figures instead of leaving them behind at the old rules.
    let previous = null;
    try {
      previous = JSON.parse(await readFile(join(dir, MANIFEST_FILE), "utf8"));
    } catch {
      previous = null;
    }
    if (previous?.generator === GENERATOR_VERSION && previous?.source === hash) {
      reportBroken(work, previous.warnings ?? []);
      say(`  kept ${rel(dir)}`);
      continue;
    }

    const d2 = await engine();
    const compiled = await d2.compile(work.source, { layout: LAYOUT });
    const boards = boardsOf(compiled.diagram, rootTitleOf(work.meta));
    const known = new Set(boards.map((board) => board.id));

    // Every link the author wrote that the compiler could not honour. d2 drops these in silence
    // and still reports success, so this is the only place a reader's dead node is visible.
    const broken = auditBoardLinks(work.source, known);
    reportBroken(work, broken);
    if (boards.length === 1) {
      console.warn(`${rel(work.file)}: \`\`\`d2 boards "${work.name}" has one board — the marker buys nothing here`);
    }
    const sequences = nonLayerKinds(compiled.diagram);
    if (sequences.length > 0) {
      console.warn(`${rel(work.file)}: "${work.name}" uses ${sequences.join(" + ")}; only \`layers\` is navigable`);
    }

    await mkdir(dir, { recursive: true });
    for (const board of boards) {
      const svg = await d2.render(board.node, renderOptions(saltForBoard(hash, board.id)));
      await writeFile(join(dir, `${board.slug}.svg`), svg, "utf8");
    }
    const manifest = manifestFor({ sourceHash: hash, boards, warnings: broken });
    await writeFile(join(dir, MANIFEST_FILE), `${JSON.stringify(manifest, null, 2)}\n`, "utf8");

    // A rename or a removed layer leaves its SVG behind; the manifest is the whole truth about
    // what this directory should hold, so anything else goes with it.
    const keep = new Set([MANIFEST_FILE, ...boards.map((board) => `${board.slug}.svg`)]);
    for (const file of await readdir(dir)) {
      if (!keep.has(file)) await unlink(join(dir, file));
    }
    say(`  drew ${rel(dir)} — ${boards.map((b) => b.slug).join(", ")}`);
  }

  // ── PRUNE THE SIDECARS ─────────────────────────────────────────────────────
  // A directory's `_d2` should hold exactly the names its markdown still declares. A directory
  // with no markdown left declares nothing, which is how a deleted lesson takes its figures.

  let stale = 0;
  for (const dir of dirs) {
    const boardsDir = join(dir, BOARDS_DIR);
    let entries;
    try {
      entries = await readdir(boardsDir);
    } catch {
      continue;
    }
    const names = declared.get(dir) ?? new Set();
    for (const entry of entries) {
      if (names.has(entry)) continue;
      stale += 1;
      if (prune) await rm(join(boardsDir, entry), { recursive: true, force: true });
      else say(`  stale ${rel(join(boardsDir, entry))} (pass --prune to remove)`);
    }
    if (prune && (await readdir(boardsDir)).length === 0) await rm(boardsDir, { recursive: true, force: true });
  }
  if (stale > 0 && prune) say(`pruned ${stale} stale walkthrough(s)`);

  if (problems > 0) {
    const line = `${problems} board link(s) name no board`;
    if (strict) throw new Error(line);
    say(`${line} — those nodes render but do not navigate`);
  }
}

// Only run when invoked directly — the test imports the contract above.
if (process.argv[1] && import.meta.url === `file://${process.argv[1]}`) {
  main().then(
    () => process.exit(0),
    (error) => {
      console.error(`render-d2: ${error?.message ?? error}`);
      process.exit(1);
    },
  );
}
