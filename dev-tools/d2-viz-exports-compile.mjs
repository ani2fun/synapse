#!/usr/bin/env node
// ── DOES THE VIZ EXPORT ACTUALLY COMPILE? ────────────────────────────────────
// `viz-wasm/src/engine/d2.rs` emits d2 source from a traced step. Its Rust goldens pin the TEXT
// byte-exact — which proves the emitter is stable and proves nothing about whether d2 can read
// it. A misplaced brace, an unquoted label, a `near:` pointed at a grid cell: all of those pass
// the Rust suite and fail in the reader's browser, silently, because a figure that will not
// compile falls back to a source placeholder rather than raising.
//
// So this compiles every checked-in export golden through the SAME wasm engine `/d2` previews
// with, and fails on the first one d2 refuses.
//
// It is a manual gate, like `d2-engines-agree.mjs` — the same standing, for the same reason:
// running the d2 engine is heavy, and neither belongs in the per-push suite.
//
// Usage:  cd web && node ../dev-tools/d2-viz-exports-compile.mjs

import { readdir, readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { join, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

const GOLDENS = resolve(import.meta.dirname, "..", "viz-wasm", "tests", "fixtures", "d2-export");

/**
 * The wasm engine, resolved from the WORKING DIRECTORY rather than from this file — the rule
 * `d2-engines-agree.mjs` follows, and for the same reason: a bare specifier resolves against the
 * importing module, and this script lives in `dev-tools/` where nothing is installed.
 */
async function loadWasmEngine() {
  const resolved = createRequire(join(process.cwd(), "package.json")).resolve("@terrastruct/d2");
  const esm = join(resolved.slice(0, resolved.indexOf(`${sep}dist${sep}`)), "dist", "node-esm", "index.js");
  return await import(pathToFileURL(esm).href);
}

const { D2 } = await loadWasmEngine();
// One instance, one call at a time — the shape `renderD2Source` uses it in, because the client
// keeps ONE in-flight request and concurrent calls on a shared instance clobber each other.
const d2 = new D2();
const compile = d2.compile.bind(d2);

const files = (await readdir(GOLDENS)).filter((f) => f.endsWith(".d2")).sort();
if (files.length === 0) {
  console.error(`d2-viz-exports-compile: no goldens under ${GOLDENS}`);
  process.exit(2);
}

let ok = 0;
let failed = 0;
for (const file of files) {
  const source = await readFile(join(GOLDENS, file), "utf8");
  try {
    const result = await compile(source, { layout: "elk" });
    // Compiling is not drawing: a board tree only reveals a bad layer when one is rendered.
    const boards = [result.diagram, ...(result.diagram.layers ?? [])];
    for (const board of boards) await d2.render(board, {});
    ok += 1;
    console.log(`  ✓ ${file} — ${boards.length} board(s)`);
  } catch (error) {
    failed += 1;
    const message = error?.message ?? String(error);
    console.error(`  ✗ ${file}\n      ${message.trim().slice(0, 400)}`);
  }
}

console.log(`\n${ok} compiled, ${failed} refused`);
process.exit(failed === 0 ? 0 : 1);
