#!/usr/bin/env node
// ── DO THE TWO d2 ENGINES STILL AGREE? ───────────────────────────────────────
// Renders every ```d2 fence in a checkout twice — once through the `d2-render` service (native Go)
// and once through `@terrastruct/d2` (wasm, in-process) — and compares the bytes.
//
// There are two engines because the reader and the author need different things. A lesson's
// figures are drawn by the service, so the page ships no engine; the `/d2` editor previews on
// every keystroke, where a round trip per character is not an option. That is a good split and a
// dangerous one: when the two drift, the editor quietly lies about what will ship, and nothing
// errors.
//
// It has drifted once already. Pinning the service to the d2 **v0.7.0 tag** — the version the wasm
// build reports — produced three extra `<mask>` rects on any diagram with a multi-line label,
// because the tag carries a bug fixed immediately after it. v0.7.2 is byte-identical.
//
// Two things are normalised away, because they are engine bookkeeping rather than picture:
//
//   `data-d2-version`  the wasm says `v0.7.0-HEAD`, the module `v0.7.1-HEAD`, for the same code.
//   `d2-<digits>`      d2's own content hash, which scopes the SVG's CSS. Measured across this
//                      catalog it differs on 9 diagrams of 101 while the element set stays
//                      identical, and each document uses exactly ONE such id throughout — so the
//                      two engines number the same picture differently rather than disagreeing
//                      about it. Counted and reported, never fatal: a page renders correctly with
//                      either number, and failing on it would make this gate cry wolf.
//
// Usage:  node dev-tools/d2-engines-agree.mjs <content-root>... [--service http://localhost:8390]

import { readdir, readFile } from "node:fs/promises";
import { join, relative, resolve } from "node:path";

import { d2Blocks, fnv1a, isBoardsFence } from "./render-d2.mjs";

const VERSION_ATTR = /\sdata-d2-version="[^"]*"/g;
const CONTENT_ID = /d2-\d{6,}/g;
const normalise = (svg) => svg.replace(VERSION_ATTR, "").replace(CONTENT_ID, "d2-ID");
/** The distinct content-hash ids one document uses. More than one would mean broken references. */
const contentIds = (svg) => new Set(svg.match(CONTENT_ID) ?? []);

async function markdownUnder(dir, out = []) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (entry.name.startsWith(".") || entry.name === "node_modules" || entry.name === "_media") continue;
    const path = join(dir, entry.name);
    if (entry.isDirectory()) await markdownUnder(path, out);
    else if (entry.name.endsWith(".md")) out.push(path);
  }
  return out;
}

const args = process.argv.slice(2);
const serviceAt = args.indexOf("--service");
const service = serviceAt === -1 ? "http://localhost:8390" : args[serviceAt + 1];
const roots = args.filter((a, i) => !a.startsWith("--") && i !== serviceAt + 1).map((r) => resolve(r));
if (roots.length === 0) {
  console.error("usage: d2-engines-agree.mjs <content-root>... [--service URL]");
  process.exit(2);
}

/**
 * The wasm engine, resolved from the WORKING DIRECTORY rather than from this file — the same rule
 * `render-d2.mjs` follows and for the same reason: a bare specifier resolves against the importing
 * module, and this script lives in `dev-tools/` where nothing is installed. Run it from `web/`.
 *
 * `require.resolve` picks the package's `require` condition, and that build exports nothing to an
 * `import()`; reach the ESM build beside it.
 */
async function loadWasmEngine() {
  const { createRequire } = await import("node:module");
  const { pathToFileURL } = await import("node:url");
  const { sep } = await import("node:path");
  const resolved = createRequire(join(process.cwd(), "package.json")).resolve("@terrastruct/d2");
  const esm = join(resolved.slice(0, resolved.indexOf(`${sep}dist${sep}`)), "dist", "node-esm", "index.js");
  return await import(pathToFileURL(esm).href);
}

// One instance, one call at a time — the shape `renderD2Source` uses it in.
const { D2 } = await loadWasmEngine();
const wasm = new D2();

let identical = 0;
let differing = 0;
let failed = 0;
let renumbered = 0;

for (const root of roots) {
  for (const file of await markdownUnder(root)) {
    for (const { source, meta } of d2Blocks(await readFile(file, "utf8"))) {
      // A walkthrough is a tree of boards addressed by slug, not one figure; `/boards` is its
      // comparison and it needs the manifest to line the two up. Out of scope here.
      if (isBoardsFence(meta)) continue;
      const hash = fnv1a(source);
      const where = `${relative(root, file)} ${hash}`;

      let drawn;
      try {
        const response = await fetch(`${service}/render`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ source }),
        });
        if (!response.ok) {
          failed += 1;
          console.error(`  ✗ ${where}: service ${response.status} ${(await response.text()).trim().slice(0, 90)}`);
          continue;
        }
        drawn = await response.text();
      } catch (error) {
        console.error(`d2-engines-agree: ${service} unreachable — ${error?.message ?? error}`);
        process.exit(1);
      }

      const compiled = await wasm.compile(source, { layout: "elk" });
      const reference = await wasm.render(compiled.diagram, {
        themeID: 0,
        pad: 20,
        noXMLTag: true,
        salt: `d2-${hash}`,
      });

      const drawnIds = contentIds(drawn);
      const referenceIds = contentIds(reference);
      // One id per document is the invariant that makes a renumbering harmless; more than one
      // would mean a reference pointing at something that is not there, which IS a broken figure.
      if (drawnIds.size > 1 || referenceIds.size > 1) {
        differing += 1;
        console.error(`  ✗ ${where}: several content ids in one document — references cannot all resolve`);
        continue;
      }
      if (normalise(drawn) !== normalise(reference)) {
        differing += 1;
        console.error(`  ✗ ${where}: ${drawn.length} B from the service, ${reference.length} B from wasm`);
        continue;
      }
      identical += 1;
      if ([...drawnIds][0] !== [...referenceIds][0]) renumbered += 1;
    }
  }
}

console.log(
  `${identical} identical · ${differing} differing · ${failed} failed` +
    (renumbered > 0 ? `  (${renumbered} same picture, different content id)` : ""),
);
// Differing output is the failure this exists to catch; a fence the service cannot draw at all is
// the same severity, because the reader gets no figure either way.
process.exit(differing + failed === 0 ? 0 : 1);
