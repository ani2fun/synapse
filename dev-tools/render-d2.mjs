#!/usr/bin/env node
// ── D2 → SVG, AHEAD OF THE REQUEST ────────────────────────────────────────────
// Walks a content checkout, compiles every ```d2 fence, and writes the SVG to
// `_media/d2/<hash>.svg`. A content repo runs this in CI and commits the result; the reader's
// page then inlines a file instead of anyone compiling anything.
//
// It lives here, in the app repo, rather than in each content repo, because the hash and the
// render options MUST match `web/src/lib/islands/diagram/d2.ts` exactly — a different salt or a
// different pad is a cache miss, and a cache miss is silent (the page falls back to the client
// renderer and simply gets slow again). `render-d2.test.ts` pins that agreement.
//
// Why not on the server: d2's Go/wasm engine peaks at ~5.2 GB of RSS rendering one 23-diagram
// lesson. That is fine on a CI runner and impossible in a 256Mi pod.
//
// Usage:  node dev-tools/render-d2.mjs <content-root> [--prune] [--quiet]

import { readdir, readFile, mkdir, writeFile, stat, unlink } from "node:fs/promises";
import { join, relative, resolve, sep } from "node:path";

// ── THE CONTRACT WITH THE RUNTIME ────────────────────────────────────────────
// Both halves of this are asserted against the app's own copies in render-d2.test.ts.

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

const FENCE = /^```(\w*)[^\n]*\n([\s\S]*?)^```/gm;

/**
 * Every ```d2 fence body in a markdown document, in order.
 *
 * Must return byte-for-byte what remark's `code` node carries, because that string is what the
 * reader hashes to find this diagram's file. Two details do the work: the language is trimmed and
 * lowercased (so ```D2 counts), and the body carries NO trailing newline — the one before the
 * closing fence belongs to the fence, not to the source. A stray "\n" here changes every hash and
 * silently misses every lookup. `renderD2Script.test.ts` parses the same markdown with remark and
 * compares.
 */
export function d2Fences(markdown) {
  const found = [];
  for (const match of markdown.matchAll(FENCE)) {
    if ((match[1] ?? "").trim().toLowerCase() === "d2") found.push(match[2].replace(/\n$/, ""));
  }
  return found;
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

async function* markdownFiles(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (entry.name.startsWith(".") || entry.name === "node_modules" || entry.name === "_media") {
      continue;
    }
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* markdownFiles(path);
    else if (entry.name.endsWith(".md")) yield path;
  }
}

// ── MAIN ─────────────────────────────────────────────────────────────────────

async function main() {
  const args = process.argv.slice(2);
  const root = resolve(args.find((a) => !a.startsWith("--")) ?? ".");
  const prune = args.includes("--prune");
  const quiet = args.includes("--quiet");
  const say = (line) => {
    if (!quiet) console.log(line);
  };

  const outDir = join(root, "_media", "d2");
  await mkdir(outDir, { recursive: true });

  // Collect first, so the engine is only started when there is something to draw.
  const wanted = new Map(); // hash → source
  let fenceCount = 0;
  for await (const file of markdownFiles(root)) {
    for (const source of d2Fences(await readFile(file, "utf8"))) {
      fenceCount += 1;
      wanted.set(fnv1a(source), source);
    }
  }
  say(`${fenceCount} d2 fence(s) across the checkout · ${wanted.size} distinct`);
  if (wanted.size === 0) return;

  // Idempotent: an unchanged diagram keeps its file, so a content edit only pays for what it
  // actually changed and the commit stays readable.
  const missing = [];
  for (const [hash, source] of wanted) {
    try {
      await stat(join(outDir, `${hash}.svg`));
    } catch {
      missing.push([hash, source]);
    }
  }
  say(`${wanted.size - missing.length} already drawn · ${missing.length} to draw`);

  if (missing.length > 0) {
    const { D2 } = await loadEngine();
    const d2 = new D2();
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
  const orphans = onDisk.filter((f) => !wanted.has(f.replace(/\.svg$/, "")));
  if (orphans.length > 0) {
    if (prune) {
      for (const file of orphans) await unlink(join(outDir, file));
      say(`pruned ${orphans.length} orphan(s)`);
    } else {
      say(`${orphans.length} orphan(s) no fence refers to (pass --prune to remove)`);
    }
  }
  say(`${relative(process.cwd(), outDir)} holds ${wanted.size} diagram(s)`);
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
