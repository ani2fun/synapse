#!/usr/bin/env node
// ── d2-interactive — a multi-board .d2 → ONE self-contained interactive page ──────────────────
// `d2 --animate-interval` packages a multi-board diagram as an SVG that cycles on a timer. This
// packages the same boards as a page the READER drives: click a node carrying `link:` to drill
// in, ◀ ▶ ⌂ to walk the boards, a menu to jump, `#board` to deep-link.
//
// The output is one file with no network at runtime — every board, the stylesheet and the viewer
// are inlined — so it opens over `file://` and survives being emailed.
//
// The board MODEL is not written here: it is `d2-boards.mjs`, the same module the CI renderer
// uses, read off disk and inlined into the page. That is what keeps a board's id, slug and title
// identical whether it was drawn for a lesson or for this. The viewer's history is deliberately
// NOT shared with the app's: in a lesson the diagram must not hijack the browser's Back button,
// and on a page that holds nothing else it should — so that part is written here, once.
//
// Usage:  node dev-tools/d2-interactive.mjs <input.d2> [--output page.html] [--title "…"]

import { readFile, writeFile } from "node:fs/promises";
import { basename, join, resolve, sep } from "node:path";

import {
  auditBoardLinks,
  boardsOf,
  manifestFor,
  nonLayerKinds,
  saltForBoard,
} from "./d2-boards.mjs";
import { LAYOUT, fnv1a, renderOptions } from "./render-d2.mjs";

// ── THE ENGINE ───────────────────────────────────────────────────────────────
// Resolved from the WORKING DIRECTORY, like `render-d2.mjs`: this script is run from wherever
// the diagrams are, and that is where the `npm i` was done.

async function loadEngine() {
  const { createRequire } = await import("node:module");
  const { pathToFileURL } = await import("node:url");
  let resolved;
  try {
    resolved = createRequire(join(process.cwd(), "package.json")).resolve("@terrastruct/d2");
  } catch {
    return await import("@terrastruct/d2");
  }
  const esm = join(resolved.slice(0, resolved.indexOf(`${sep}dist${sep}`)), "dist", "node-esm", "index.js");
  return await import(pathToFileURL(esm).href);
}

// ── THE PAGE ─────────────────────────────────────────────────────────────────

/** Safe to drop inside a `<script>`: the one sequence that could end it early is escaped. */
const jsonInScript = (value) => JSON.stringify(value).replace(/</g, "\\u003c");

const escapeHtml = (text) =>
  String(text).replace(
    /[&<>"]/g,
    (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[ch],
  );

const STYLE = `
:root { color-scheme: light dark; --ink: #0a0f25; --dim: #676c7e; --line: #e2e8f0;
        --bg: #f6f7f9; --card: #ffffff; --accent: #0f9d8f; }
@media (prefers-color-scheme: dark) {
  :root { --ink: #e8eaf0; --dim: #9aa1b2; --line: #2b3040; --bg: #14171f; --card: #1b1f2a; }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink); font: 15px/1.5 ui-sans-serif,
       system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }
main { max-width: 76rem; margin: 0 auto; padding: 18px 20px 28px; }
h1 { margin: 0 0 4px; font-size: 1.15rem; }
.sub { margin: 0 0 14px; color: var(--dim); font-size: 0.85rem; }
.bar { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin-bottom: 10px; }
button { font: inherit; cursor: pointer; border: 1px solid var(--line); border-radius: 8px;
         background: var(--card); color: var(--ink); padding: 5px 10px; }
button:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }
button:disabled { opacity: 0.4; cursor: default; }
button:focus-visible, a:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.trail { display: flex; align-items: center; gap: 2px; flex-wrap: wrap; font-size: 0.85rem; }
.trail .sep { color: var(--dim); margin: 0 4px; }
.trail .here { font-weight: 600; }
.trail button { border: 0; background: none; color: var(--dim); padding: 2px 4px; }
.menu-wrap { position: relative; margin-left: auto; }
.menu { position: absolute; right: 0; top: calc(100% + 6px); z-index: 5; margin: 0; padding: 4px;
        list-style: none; min-width: 12rem; max-height: 60vh; overflow-y: auto;
        border: 1px solid var(--line); border-radius: 10px; background: var(--card);
        box-shadow: 0 12px 32px -14px rgba(0,0,0,0.5); }
.menu button { display: block; width: 100%; text-align: left; border: 0; background: none;
               padding: 6px 10px; }
.menu button[aria-current="true"] { color: var(--accent); font-weight: 600; }
.stage { position: relative; border: 1px solid var(--line); border-radius: 12px;
         background: #ffffff; overflow: hidden; height: min(72vh, 44rem); }
.viewport { width: 100%; height: 100%; overflow: hidden; cursor: grab; touch-action: none; }
.viewport:active { cursor: grabbing; }
.figure { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center;
          transform-origin: center center; }
.figure svg { display: block; width: auto; height: auto; max-width: 100%; max-height: 100%;
              animation: in 220ms ease; }
@keyframes in { from { opacity: 0 } to { opacity: 1 } }
@media (prefers-reduced-motion: reduce) { .figure svg { animation: none } }
.zoom { position: absolute; bottom: 14px; left: 50%; transform: translateX(-50%); display: flex;
        gap: 6px; padding: 5px 7px; border: 1px solid var(--line); border-radius: 999px;
        background: var(--card); }
.zoom span { min-width: 3.2rem; text-align: center; font: 12px ui-monospace, monospace;
             color: var(--dim); align-self: center; }
.warn { margin-top: 12px; padding: 8px 12px; border-radius: 8px; font-size: 0.85rem;
        border: 1px solid #d97706; background: rgba(217,119,6,0.1); color: #b45309; }
.warn ul { margin: 6px 0 0; padding-left: 18px; }
.sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
`;

/** The viewer. Plain DOM, no framework — it has to run from a `file://` URL with nothing else. */
const VIEWER = `
(function () {
  var data = JSON.parse(document.getElementById("d2-manifest").textContent);
  var boards = data.boards;
  var byId = {}, bySlug = {};
  boards.forEach(function (b) { byId[b.id] = b; bySlug[b.slug] = b; });

  var figure = document.querySelector(".figure");
  var trail = document.querySelector(".trail");
  var menu = document.querySelector(".menu");
  var live = document.querySelector(".sr");
  var btnBack = document.getElementById("back"), btnFwd = document.getElementById("fwd");
  var btnHome = document.getElementById("home"), btnMenu = document.getElementById("menu-btn");

  // Board history. Unlike the in-app viewer this DOES own the browser's Back button: there is
  // nothing else on this page for it to mean.
  var stack = [], at = -1, scale = 1, pan = { x: 0, y: 0 };

  function svgFor(id) {
    var node = document.getElementById("board-" + byId[id].slug);
    return node ? node.innerHTML : "";
  }

  function paint() {
    var board = byId[stack[at]];
    figure.innerHTML = svgFor(board.id);
    scale = 1; pan = { x: 0, y: 0 }; applyTransform();

    trail.textContent = "";
    var chain = [], walk = board, guard = 0;
    while (walk && guard++ < boards.length) { chain.unshift(walk); walk = walk.parent ? byId[walk.parent] : null; }
    chain.forEach(function (crumb, i) {
      if (i > 0) { var sep = document.createElement("span"); sep.className = "sep"; sep.textContent = "\\u203a"; trail.appendChild(sep); }
      if (crumb.id === board.id) {
        var here = document.createElement("span"); here.className = "here"; here.textContent = crumb.title; trail.appendChild(here);
      } else {
        var b = document.createElement("button"); b.textContent = crumb.title;
        b.onclick = function () { go(crumb.id, true); }; trail.appendChild(b);
      }
    });

    btnBack.disabled = !canStep(-1);
    btnFwd.disabled = !canStep(1);
    btnHome.disabled = board.id === data.root;
    menu.querySelectorAll("button").forEach(function (item) {
      item.setAttribute("aria-current", String(item.dataset.board === board.id));
    });
    live.textContent = board.title;
    document.title = board.title + " \\u2014 " + data.name;
  }

  function go(id, push) {
    if (!byId[id]) return;
    if (stack[at] === id) return;
    stack = stack.slice(0, at + 1); stack.push(id); at = stack.length - 1;
    if (push !== false) history.pushState({ id: id }, "", "#" + byId[id].slug);
    paint();
  }

  // The board one step away in walk order, or null at either end. History alone leaves both
  // arrows disabled on a freshly opened page — it has been nowhere — which reads as broken
  // controls, so stepping is the fallback and walks the boards in the order the manifest lists them.
  function neighbour(delta) {
    for (var i = 0; i < boards.length; i++) {
      if (boards[i].id === stack[at]) {
        var next = boards[i + delta];
        return next ? next.id : null;
      }
    }
    return null;
  }

  function canStep(delta) {
    var next = at + delta;
    if (next >= 0 && next < stack.length) return true;
    return neighbour(delta) != null;
  }

  function step(delta) {
    var next = at + delta;
    if (next >= 0 && next < stack.length) {
      at = next;
      history.pushState({ id: stack[at] }, "", "#" + byId[stack[at]].slug);
      paint();
      return;
    }
    // History wins wherever it exists; only when it is spent does the arrow walk the boards.
    var sideways = neighbour(delta);
    if (sideways) go(sideways, true);
  }

  function applyTransform() {
    figure.style.transform = "translate(" + pan.x + "px," + pan.y + "px) scale(" + scale.toFixed(3) + ")";
  }
  function zoomBy(f) { scale = Math.min(Math.max(scale * f, 0.25), 4); applyTransform(); document.getElementById("level").textContent = Math.round(scale * 100) + "%"; }

  // A click on a node d2 linked to another board. The href IS the board id — the compiler writes
  // the absolute path — so this is a lookup, not a parse.
  figure.addEventListener("click", function (event) {
    var a = event.target.closest && event.target.closest("a");
    if (!a) return;
    var href = a.getAttribute("xlink:href") || a.getAttribute("href") || "";
    if (/^[a-z][a-z0-9+.-]*:/i.test(href)) { a.target = "_blank"; a.rel = "noopener noreferrer"; return; }
    event.preventDefault();
    if (byId[href]) go(href, true);
  });

  btnBack.onclick = function () { step(-1); };
  btnFwd.onclick = function () { step(1); };
  btnHome.onclick = function () { go(data.root, true); };
  btnMenu.onclick = function () { menu.hidden = !menu.hidden; btnMenu.setAttribute("aria-expanded", String(!menu.hidden)); };
  menu.querySelectorAll("button").forEach(function (item) {
    item.onclick = function () { menu.hidden = true; btnMenu.setAttribute("aria-expanded", "false"); go(item.dataset.board, true); };
  });

  document.getElementById("zin").onclick = function () { zoomBy(1.25); };
  document.getElementById("zout").onclick = function () { zoomBy(1 / 1.25); };
  document.getElementById("zreset").onclick = function () { scale = 1; pan = { x: 0, y: 0 }; applyTransform(); document.getElementById("level").textContent = "100%"; };

  var viewport = document.querySelector(".viewport"), grip = null;
  viewport.addEventListener("pointerdown", function (e) { e.preventDefault(); grip = { x: e.clientX, y: e.clientY }; });
  window.addEventListener("pointermove", function (e) {
    if (!grip) return;
    pan.x += e.clientX - grip.x; pan.y += e.clientY - grip.y;
    grip = { x: e.clientX, y: e.clientY }; applyTransform();
  });
  window.addEventListener("pointerup", function () { grip = null; });
  viewport.addEventListener("wheel", function (e) { e.preventDefault(); zoomBy(e.deltaY < 0 ? 1.12 : 1 / 1.12); }, { passive: false });

  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape" && !menu.hidden) { menu.hidden = true; btnMenu.setAttribute("aria-expanded", "false"); return; }
    if (e.target && /^(INPUT|TEXTAREA)$/.test(e.target.tagName)) return;
    if (e.key === "ArrowLeft") step(-1);
    else if (e.key === "ArrowRight") step(1);
    else if (e.key === "Home") go(data.root, true);
    else return;
    e.preventDefault();
  });

  // The browser's own Back/Forward, and a #board deep link on first load.
  window.addEventListener("popstate", function (event) {
    var id = (event.state && event.state.id) || fromHash() || data.root;
    if (!byId[id]) id = data.root;
    var found = stack.indexOf(id);
    if (found >= 0) at = found; else { stack.push(id); at = stack.length - 1; }
    paint();
  });
  function fromHash() {
    var slug = decodeURIComponent((location.hash || "").replace(/^#/, ""));
    return bySlug[slug] ? bySlug[slug].id : null;
  }

  var opening = fromHash() || data.root;
  stack = [opening]; at = 0;
  history.replaceState({ id: opening }, "", location.hash || "");
  paint();
})();
`;

function page({ title, name, manifest, boards, svgs, warnings }) {
  const menuItems = boards
    .map((b) => `<li><button data-board="${escapeHtml(b.id)}">${escapeHtml(b.title)}</button></li>`)
    .join("");
  const stash = boards
    .map((b) => `<div id="board-${escapeHtml(b.slug)}" hidden>${svgs.get(b.id)}</div>`)
    .join("\n");
  const warn =
    warnings.length === 0
      ? ""
      : `<div class="warn"><strong>${warnings.length} link(s) name no board</strong> — those nodes render but do not navigate.<ul>` +
        warnings
          .map(
            (w) =>
              `<li>line ${w.line}: <code>link: ${escapeHtml(w.value)}</code> in ${escapeHtml(w.board)}` +
              (w.hint ? ` — did you mean <code>${escapeHtml(w.hint)}</code>?` : "") +
              `</li>`,
          )
          .join("") +
        `</ul></div>`;

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)}</title>
<style>${STYLE}</style>
</head>
<body>
<main>
  <h1>${escapeHtml(title)}</h1>
  <p class="sub">${boards.length} boards · click a linked node to drill in · ← → to step, Home for the root</p>
  <div class="bar">
    <button id="back" aria-label="Back" title="Back">‹</button>
    <button id="fwd" aria-label="Forward" title="Forward">›</button>
    <button id="home" aria-label="Root board" title="Root board">⌂</button>
    <nav class="trail" aria-label="Breadcrumb"></nav>
    <div class="menu-wrap">
      <button id="menu-btn" aria-haspopup="true" aria-expanded="false" aria-label="Jump to a board">☰</button>
      <ul class="menu" hidden>${menuItems}</ul>
    </div>
  </div>
  <div class="stage">
    <div class="viewport"><div class="figure"></div></div>
    <div class="zoom">
      <button id="zout" aria-label="Zoom out">−</button>
      <span id="level">100%</span>
      <button id="zin" aria-label="Zoom in">+</button>
      <button id="zreset" aria-label="Reset zoom">⟲</button>
    </div>
  </div>
  <p class="sr" aria-live="polite"></p>
  ${warn}
</main>
${stash}
<script type="application/json" id="d2-manifest">${jsonInScript({ ...manifest, name })}</script>
<script>${VIEWER}</script>
</body>
</html>
`;
}

// ── MAIN ─────────────────────────────────────────────────────────────────────

async function main() {
  const args = process.argv.slice(2);
  const positional = args.filter((a) => !a.startsWith("--"));
  const flag = (name) => {
    const at = args.indexOf(`--${name}`);
    return at >= 0 ? args[at + 1] : undefined;
  };
  const input = positional[0];
  if (input == null) {
    throw new Error("usage: d2-interactive <input.d2> [--output page.html] [--title \"…\"]");
  }
  const inputPath = resolve(input);
  const stem = basename(inputPath).replace(/\.d2$/i, "");
  const output = resolve(flag("output") ?? `${stem}.html`);
  const title = flag("title") ?? stem;

  const source = await readFile(inputPath, "utf8");
  const { D2 } = await loadEngine();
  const d2 = new D2();
  const compiled = await d2.compile(source, { layout: LAYOUT });
  const boards = boardsOf(compiled.diagram, flag("root") ?? title);
  if (boards.length === 1) {
    console.warn(`${input}: this diagram has one board — there is nothing to navigate between`);
  }
  const sequences = nonLayerKinds(compiled.diagram);
  if (sequences.length > 0) {
    console.warn(`${input}: uses ${sequences.join(" + ")}; only \`layers\` is navigable`);
  }

  const hash = fnv1a(source);
  const known = new Set(boards.map((b) => b.id));
  const warnings = auditBoardLinks(source, known);
  for (const miss of warnings) {
    const hint = miss.hint ? ` — did you mean \`link: ${miss.hint}\`?` : "";
    console.warn(`${input}:${miss.line}: \`link: ${miss.value}\` in ${miss.board} names no board${hint}`);
  }

  // One at a time: the engine serves a single request, so overlapping renders hang it.
  const svgs = new Map();
  for (const board of boards) {
    svgs.set(board.id, await d2.render(board.node, renderOptions(saltForBoard(hash, board.id))));
  }

  const manifest = manifestFor({ sourceHash: hash, boards, warnings });
  await writeFile(output, page({ title, name: title, manifest, boards, svgs, warnings }), "utf8");
  const kb = Math.round((await readFile(output)).length / 1024);
  console.log(`${output} — ${boards.length} board(s), ${kb} KB, no network at runtime`);
}

if (process.argv[1] && import.meta.url === `file://${process.argv[1]}`) {
  main().then(
    () => process.exit(0),
    (error) => {
      console.error(`d2-interactive: ${error?.message ?? error}`);
      process.exit(1);
    },
  );
}
