import * as log from "../lib/log";
// The reader's floating chrome. Vanilla TS — the SSR page is plain HTML and every job here is
// either `localStorage` (the sidebar face, the prefs pack) or a scroll/click listener building
// fixed-position affordances that aren't in the server markup. There is nothing for a component
// framework to hydrate INTO.
//
// This island loads ONLY on lesson pages (the `[...path].astro` `else` branch — problem pages get
// none of it). Three pieces, each a section below:
//   1. the sidebar's three persisted faces (Expanded · Compact rail · Hidden) + the collapse
//      controls;
//   2. the reading-preferences FAB + popover — the pure parse/serialize/applyToHtml half lives in
//      `lib/catalog/prefs.ts`;
//   3. the right-edge minimap + the TOC FAB/popover, fed by one scroll recompute (progress +
//      active heading) over headings harvested from `.lesson-body`.
//
// Not implemented here: the sidebar Filter box and the ← Learn browse toggle — both belong to the
// Expanded face but are their own feature and no e2e spec exercises them; and the sticky
// wayfinding bar, the 2px top progress bar, focus mode, and the scroll-to-top FAB — separate
// chrome outside this pass's scope. The minimap needs a progress fraction regardless, so that one
// scroll value is computed.

import * as storage from "../lib/storage";
import {
  DEFAULT_PREFS,
  FAMILIES,
  LEADINGS,
  SIZES,
  WIDTHS,
  applyToHtml,
  parse as parsePrefs,
  serialize as serializePrefs,
  type Prefs,
} from "../lib/catalog/prefs";
import { spreadFractions } from "../lib/catalog/chrome";

/** A harvested prose heading (h2/h3 with an id — rehype-slug mints them). */
interface Heading {
  id: string;
  text: string;
  level: number;
}

/** Build an SVG icon element from its inner path markup. */
function icon(cls: string, inner: string): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", cls);
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  svg.innerHTML = inner;
  return svg;
}

// ─────────────────────────────────────────────────────────────────────────────
// SIDEBAR FACES
// ─────────────────────────────────────────────────────────────────────────────

type Face = "expanded" | "compact" | "hidden";

/** Parse the stored face token — `collapsed` is a legacy alias for `hidden`, kept so a visitor's
 *  previously saved face still carries over. */
function parseFace(token: string | null): Face {
  if (token === "compact") return "compact";
  if (token === "hidden" || token === "collapsed") return "hidden";
  return "expanded";
}

/** One top-level chapter distilled from the SSR sidebar tree, for the Compact rail. */
interface RailChapter {
  name: string;
  href: string | null;
  active: boolean;
}

/** Read the top-level chapters straight off the rendered `.reader-sidebar__tree`, tiling a book's
 *  top-level CHAPTERS only; a top-level lesson row — or a namesake-collapsed chapter the tree
 *  flattened to a lesson row — gets no tile. This walks the RAW `book.entries`, so a
 *  namesake-collapsed top-level chapter would still number there; such chapters are problem dirs
 *  that never sit at a book's top level in this content, so it doesn't come up in practice. */
function railChapters(sidebar: Element): RailChapter[] {
  const tree = sidebar.querySelector(".reader-sidebar__tree");
  if (!tree) return [];
  const out: RailChapter[] = [];
  for (const li of Array.from(tree.children)) {
    const details = li.querySelector(":scope > details.reader-sidebar__section");
    if (!details) continue;
    const name = details.querySelector(".reader-sidebar__name")?.textContent?.trim() ?? "";
    const firstLink = details.querySelector<HTMLAnchorElement>("a.reader-sidebar__link");
    out.push({
      name,
      href: firstLink?.getAttribute("href") ?? null,
      active: !!details.querySelector(".reader-sidebar__link--active"),
    });
  }
  return out;
}

/** Two decorative left dots on `pad2` — `"1"` → `"01"`. */
function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

/** Build the Compact rail once from the tree: an expand button atop a column of numbered tiles,
 *  each linking its chapter's first lesson; the active tile carries the conic `--progress` ring.
 *  Returns the rail plus the active tile (so scroll can drive its ring). */
function buildRail(sidebar: Element, onExpand: () => void): { rail: HTMLElement; activeTile: HTMLElement | null } {
  const rail = document.createElement("div");
  rail.className = "reader-rail";

  const expand = document.createElement("button");
  expand.className = "reader-rail__expand";
  expand.setAttribute("aria-label", "Expand the sidebar");
  expand.type = "button";
  expand.append(icon("reader-rail__expand-ic", '<rect width="18" height="18" x="3" y="3" rx="2"></rect><path d="M9 3v18 M14 9l3 3-3 3"></path>'));
  expand.addEventListener("click", onExpand);
  rail.append(expand);

  const tiles = document.createElement("div");
  tiles.className = "reader-rail__tiles";
  let activeTile: HTMLElement | null = null;
  railChapters(sidebar).forEach((chapter, i) => {
    const label = pad2(i + 1);
    const tooltip = `${label} · ${chapter.name}`;
    const num = document.createElement("span");
    num.className = "reader-rail__num";
    num.textContent = label;
    let tile: HTMLElement;
    if (chapter.href) {
      const a = document.createElement("a");
      a.href = chapter.href;
      tile = a;
    } else {
      tile = document.createElement("span");
    }
    tile.className = chapter.active ? "reader-rail__tile reader-rail__tile--active" : "reader-rail__tile";
    tile.title = tooltip;
    tile.append(num);
    if (chapter.active) activeTile = tile;
    tiles.append(tile);
  });
  rail.append(tiles);
  return { rail, activeTile };
}

/** The Expanded face's toprow — the collapse controls the SSR inner never carried
 *  (`.reader-sidebar__controls`, the two `.reader-sidebar__hide` buttons). */
function buildToprow(toCompact: () => void, toHidden: () => void): HTMLElement {
  const row = document.createElement("div");
  row.className = "reader-sidebar__toprow";
  const controls = document.createElement("div");
  controls.className = "reader-sidebar__controls";

  const compactBtn = document.createElement("button");
  compactBtn.className = "reader-sidebar__hide";
  compactBtn.type = "button";
  compactBtn.title = "Collapse to a rail";
  compactBtn.append(icon("reader-sidebar__hide-ic", '<path d="m11 17-5-5 5-5 M18 17l-5-5 5-5"></path>'));
  compactBtn.addEventListener("click", toCompact);

  const hideBtn = document.createElement("button");
  hideBtn.className = "reader-sidebar__hide";
  hideBtn.type = "button";
  hideBtn.title = "Hide the sidebar";
  hideBtn.append(icon("reader-sidebar__hide-ic", '<rect width="18" height="18" x="3" y="3" rx="2"></rect><path d="M9 3v18 M16 15l-3-3 3-3"></path>'));
  hideBtn.addEventListener("click", toHidden);

  controls.append(compactBtn, hideBtn);
  row.append(controls);
  return row;
}

/** The floating expand affordance for the Hidden face. Kept in the DOM always;
 *  `.reader-expand--hidden` toggles it (CSS: display:none below 1024px, inline-flex above —
 *  desktop-only, the mobile route back is the drawer FAB). */
function buildFloatingExpand(onExpand: () => void): HTMLButtonElement {
  const btn = document.createElement("button");
  btn.className = "reader-expand reader-expand--hidden";
  btn.type = "button";
  btn.setAttribute("aria-label", "Show the sidebar");
  btn.append(icon("reader-expand__ic", '<rect width="18" height="18" x="3" y="3" rx="2"></rect><path d="M9 3v18 M14 9l3 3-3 3"></path>'));
  btn.addEventListener("click", onExpand);
  return btn;
}

interface Faces {
  /** Update the active Compact-rail tile's progress ring (0–1); no-op when not in Compact. */
  setRailProgress: (fraction: number) => void;
}

function wireSidebarFaces(layout: HTMLElement): Faces {
  const sidebar = layout.querySelector<HTMLElement>(".reader-sidebar");
  const inner = sidebar?.querySelector<HTMLElement>(".reader-sidebar__inner");
  if (!sidebar || !inner) return { setRailProgress: () => {} };

  let face: Face = parseFace(storage.get(storage.READER_SIDEBAR_KEY));

  const set = (next: Face): void => {
    face = next;
    storage.set(storage.READER_SIDEBAR_KEY, next);
    apply();
    log.info(`chrome: sidebar face → ${next}`);
  };

  inner.prepend(
    buildToprow(
      () => set("compact"),
      () => set("hidden"),
    ),
  );
  const { rail, activeTile } = buildRail(sidebar, () => set("expanded"));
  rail.style.display = "none";
  sidebar.append(rail);
  const floating = buildFloatingExpand(() => set("expanded"));
  document.body.append(floating);

  const setRailProgress = (fraction: number): void => {
    if (face === "compact" && activeTile) {
      activeTile.style.setProperty("--progress", (fraction * 100).toFixed(1));
    }
  };
  const apply = (): void => {
    layout.setAttribute("data-sidebar", face);
    inner.style.display = face === "compact" ? "none" : "";
    rail.style.display = face === "compact" ? "" : "none";
    floating.classList.toggle("reader-expand--hidden", face !== "hidden");
    // Seed the ring from the current scroll position so switching to Compact shows it at once,
    // not only after the next scroll event.
    if (face === "compact") {
      const scrollable = document.documentElement.scrollHeight - window.innerHeight;
      setRailProgress(scrollable > 0 ? Math.min(Math.max(window.scrollY / scrollable, 0), 1) : 0);
    }
  };
  apply();
  log.debug(`chrome: sidebar face restored as ${face}`);

  return { setRailProgress };
}

// ─────────────────────────────────────────────────────────────────────────────
// READING-PREFERENCES FAB + POPOVER
// ─────────────────────────────────────────────────────────────────────────────

/** A three-way segmented control. `preview` renders each option in the font its token names
 *  (`--serif/--sans/--mono`). Reflects + persists on click. */
function segmented(
  label: string,
  options: readonly [string, string][],
  field: keyof Prefs,
  preview: boolean,
  read: () => Prefs,
  write: (next: Prefs) => void,
): { group: HTMLElement; refresh: () => void } {
  const group = document.createElement("div");
  group.className = "reader-prefs__group";
  const labelEl = document.createElement("div");
  labelEl.className = "reader-prefs__label";
  labelEl.textContent = label;
  const seg = document.createElement("div");
  seg.className = "reader-prefs__seg";

  const buttons: { token: string; el: HTMLButtonElement }[] = [];
  for (const [token, display] of options) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = preview ? `reader-prefs__opt reader-prefs__opt--${token}` : "reader-prefs__opt";
    btn.textContent = display;
    btn.addEventListener("click", () => {
      write({ ...read(), [field]: token });
      log.debug(`chrome: prefs ${field} → ${token}`);
    });
    seg.append(btn);
    buttons.push({ token, el: btn });
  }
  group.append(labelEl, seg);

  const refresh = (): void => {
    const current = read()[field];
    for (const { token, el } of buttons) {
      el.classList.toggle("reader-prefs__opt--active", token === current);
    }
  };
  return { group, refresh };
}

function wirePrefsFab(): void {
  const wrap = document.createElement("div");
  wrap.className = "reader-prefs";

  const fab = document.createElement("button");
  fab.className = "reader-prefs-fab";
  fab.type = "button";
  fab.title = "Reading preferences";
  fab.setAttribute("aria-label", "Reading preferences");
  fab.textContent = "Aa";

  let prefs: Prefs = parsePrefs(storage.get(storage.READER_PREFS_KEY));
  const read = (): Prefs => prefs;

  const specs: [string, readonly [string, string][], keyof Prefs, boolean][] = [
    ["Size", SIZES, "size", false],
    ["Leading", LEADINGS, "leading", false],
    ["Type family", FAMILIES, "family", true],
    ["Width", WIDTHS, "width", false],
  ];

  // The pop is built on open and removed on close, NOT toggled with `hidden` — the class rule
  // carries `display: flex`, which outranks `[hidden]`'s UA `display:none`.
  let scrim: HTMLDivElement | null = null;
  let pop: HTMLDivElement | null = null;
  const close = (): void => {
    scrim?.remove();
    pop?.remove();
    scrim = null;
    pop = null;
  };
  const open = (): void => {
    if (pop) return;
    scrim = document.createElement("div");
    scrim.className = "reader-prefs-scrim";
    scrim.addEventListener("click", close);

    pop = document.createElement("div");
    pop.className = "reader-prefs-pop";
    const eyebrow = document.createElement("div");
    eyebrow.className = "reader-prefs-pop__eyebrow";
    eyebrow.textContent = "Reading preferences";
    pop.append(eyebrow);
    const groups: { refresh: () => void }[] = [];
    const commit = (next: Prefs): void => {
      prefs = next;
      applyToHtml(next);
      storage.set(storage.READER_PREFS_KEY, serializePrefs(next));
      for (const g of groups) g.refresh();
    };
    for (const [label, options, field, preview] of specs) {
      const control = segmented(label, options, field, preview, read, commit);
      groups.push(control);
      pop.append(control.group);
    }
    const reset = document.createElement("button");
    reset.className = "reader-prefs__reset";
    reset.type = "button";
    reset.textContent = "Reset to defaults";
    reset.addEventListener("click", () => {
      commit(DEFAULT_PREFS);
      log.info("chrome: prefs reset to defaults");
    });
    pop.append(reset);
    for (const g of groups) g.refresh();

    wrap.append(scrim, pop);
    log.info("chrome: prefs pane open");
  };
  fab.addEventListener("click", () => (pop ? close() : open()));
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && pop) close();
  });

  wrap.append(fab);
  document.body.append(wrap);
}

// ─────────────────────────────────────────────────────────────────────────────
// MINIMAP + TOC, fed by one scroll recompute
// ─────────────────────────────────────────────────────────────────────────────

/** Jump to a heading, offset for the fixed header (−80px). */
function scrollToHeading(id: string): void {
  const el = document.getElementById(id);
  if (!el) return;
  const top = el.getBoundingClientRect().top + window.scrollY - 80;
  window.scrollTo(0, top);
}

/** Harvest h2[id]/h3[id] from the rendered prose (the leading h1 is the page title and has no
 *  id). */
function harvestHeadings(body: Element): Heading[] {
  const out: Heading[] = [];
  body.querySelectorAll<HTMLElement>("h2[id], h3[id]").forEach((el) => {
    const id = el.getAttribute("id");
    if (!id) return;
    out.push({ id, text: el.textContent ?? "", level: el.tagName.toLowerCase() === "h3" ? 3 : 2 });
  });
  return out;
}

function wireMinimapAndToc(headings: Heading[], onProgress: (fraction: number) => void): void {
  // ── the minimap — one tick per heading at its de-overlapped fraction ──
  const minimap = document.createElement("aside");
  minimap.className = "reader-minimap";
  minimap.setAttribute("aria-label", "Section map");
  const track = document.createElement("div");
  track.className = "reader-minimap__track";
  const fill = document.createElement("div");
  fill.className = "reader-minimap__fill";
  track.append(fill);
  minimap.append(track);

  const gap = Math.min(0.05, 1.0 / (headings.length + 1));
  const tickEls: { id: string; el: HTMLButtonElement }[] = [];
  const measure = (): void => {
    if (headings.length <= 1) return;
    const total = Math.max(document.documentElement.scrollHeight, 1);
    const scroll = window.scrollY;
    const raw = headings.map((h) => {
      const el = document.getElementById(h.id);
      return el ? (el.getBoundingClientRect().top + scroll) / total : NaN;
    });
    if (raw.some(Number.isNaN)) return;
    const spread = spreadFractions(raw);
    headings.forEach((h, i) => {
      const btn = tickEls[i]?.el;
      if (btn) btn.style.top = `${(spread[i] * 100).toFixed(2)}%`;
    });
  };
  if (headings.length > 1) {
    headings.forEach((h) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = h.level === 3 ? "reader-minimap__tick reader-minimap__tick--l3" : "reader-minimap__tick reader-minimap__tick--l2";
      btn.style.height = `min(18px, ${(gap * 100).toFixed(2)}%)`;
      btn.setAttribute("aria-label", `Jump to ${h.text}`);
      btn.addEventListener("click", () => scrollToHeading(h.id));
      const lbl = document.createElement("span");
      lbl.className = "reader-minimap__label";
      lbl.textContent = h.text;
      btn.append(lbl);
      track.append(btn);
      tickEls.push({ id: h.id, el: btn });
    });
    document.body.append(minimap);
    measure();
    window.addEventListener("resize", measure);
    log.debug(`chrome: minimap ${headings.length} ticks`);
  }

  // ── the TOC FAB + popover ──
  const fab = document.createElement("button");
  fab.className = "reader-toc-fab";
  fab.type = "button";
  fab.setAttribute("aria-label", "On this page");
  fab.setAttribute("aria-expanded", "false");
  fab.append(icon("reader-toc-fab__icon", '<path d="M8 6h13 M8 12h13 M8 18h13 M3 6h.01 M3 12h.01 M3 18h.01"></path>'));

  const rows: { id: string; row: HTMLLIElement }[] = [];
  let scrim: HTMLDivElement | null = null;
  let pop: HTMLDivElement | null = null;
  const close = (): void => {
    scrim?.remove();
    pop?.remove();
    scrim = null;
    pop = null;
    fab.setAttribute("aria-expanded", "false");
  };
  const open = (): void => {
    if (pop) return;
    scrim = document.createElement("div");
    scrim.className = "reader-toc-scrim";
    scrim.addEventListener("click", close);
    pop = document.createElement("div");
    pop.className = "reader-toc-pop";
    const eyebrow = document.createElement("div");
    eyebrow.className = "reader-toc-pop__eyebrow";
    eyebrow.textContent = "On this page";
    const list = document.createElement("ul");
    list.className = "reader-toc-pop__list";
    rows.length = 0;
    for (const h of headings) {
      const li = document.createElement("li");
      li.className = "reader-toc-pop__row";
      const a = document.createElement("a");
      a.href = `#${h.id}`;
      a.className = h.level >= 3 ? "reader-toc-pop__btn reader-toc-pop__btn--l3" : "reader-toc-pop__btn";
      a.addEventListener("click", (event) => {
        event.preventDefault();
        scrollToHeading(h.id);
        close();
      });
      const tick = document.createElement("span");
      tick.className = "reader-toc-pop__tick";
      const lbl = document.createElement("span");
      lbl.className = "reader-toc-pop__label";
      lbl.textContent = h.text;
      a.append(tick, lbl);
      li.append(a);
      list.append(li);
      rows.push({ id: h.id, row: li });
    }
    pop.append(eyebrow, list);
    document.body.append(scrim, pop);
    fab.setAttribute("aria-expanded", "true");
    reflectActive();
    log.info("chrome: TOC open");
  };
  fab.addEventListener("click", () => (pop ? close() : open()));
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && pop) close();
  });
  document.body.append(fab);

  // ── the one scroll recompute ──
  let activeId: string | null = null;
  const reflectActive = (): void => {
    for (const { id, el } of tickEls) {
      el.classList.toggle("reader-minimap__tick--active", id === activeId);
    }
    for (const { id, row } of rows) {
      row.classList.toggle("reader-toc-pop__row--active", id === activeId);
    }
  };
  const recompute = (): void => {
    const track2 = document.documentElement.scrollHeight - window.innerHeight;
    const scroll = window.scrollY;
    const fraction = track2 > 0 ? Math.min(Math.max(scroll / track2, 0), 1) : 0;
    fill.style.height = `${(fraction * 100).toFixed(2)}%`;
    onProgress(fraction);
    let next: string | null = null;
    for (const h of headings) {
      const el = document.getElementById(h.id);
      if (el && el.getBoundingClientRect().top <= 120) next = h.id;
    }
    activeId = next ?? headings[0]?.id ?? null;
    reflectActive();
  };
  recompute();
  window.addEventListener("scroll", recompute, { passive: true });
}

// ─────────────────────────────────────────────────────────────────────────────
// INIT
// ─────────────────────────────────────────────────────────────────────────────

function init(): void {
  // Belt-and-braces: this island is only imported in the lesson branch, but a `.pwb[data-problem]`
  // guard keeps it a no-op if that ever changes (problem pages show NONE of this chrome).
  if (document.querySelector(".pwb[data-problem]")) return;
  const layout = document.querySelector<HTMLElement>(".reader-layout");
  if (!layout) return;
  log.info("chrome: island init");

  const faces = wireSidebarFaces(layout);
  wirePrefsFab();

  const body = document.querySelector(".lesson-body");
  const headings = body ? harvestHeadings(body) : [];
  log.debug(`chrome: harvested ${headings.length} headings`);
  wireMinimapAndToc(headings, faces.setRailProgress);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
