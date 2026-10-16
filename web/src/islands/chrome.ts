import * as log from "../lib/log";
// The reader's floating chrome. Vanilla TS — the SSR page is plain HTML and every job here is
// either `localStorage` (the sidebar face) or a scroll/click listener building fixed-position
// affordances that aren't in the server markup. There is nothing for a component framework to
// hydrate INTO.
//
// This island loads ONLY on lesson pages (the `[...path].astro` `else` branch — problem pages get
// none of it). Three pieces:
//   1. the sidebar's three persisted faces (Expanded · Compact rail · Hidden) + the collapse
//      controls, a section below;
//   2. the on-this-page outline, fed by one scroll pass over the headings harvested from
//      `.lesson-body`: it renders into the desktop rail (`.reader-outline`), drives the thin top
//      reading-progress bar, and reveals the scroll-to-top FAB — a section below;
//   3. the reader menu (`islands/reader/menu`): Contents, Search Book, Themes & Settings, and the
//      vertical scrubber that moves through this page while the menu is open.
//
// Not implemented here: the sidebar Filter box and the ← Learn browse toggle — both belong to the
// Expanded face but are their own feature and no e2e spec exercises them; and the sticky
// wayfinding bar and focus mode — separate chrome outside this pass's scope.

import * as storage from "../lib/storage";
import { harvestHeadings, icon, scrollToHeading, type Heading } from "./reader/headings";
import { wireMenu } from "./reader/menu";

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

/** The Expanded face's collapse controls (`.reader-sidebar__controls`, the two
 *  `.reader-sidebar__hide` buttons). Appended into the sidebar's SSR toprow, which already carries
 *  the "Back to Main" link — so the back link and the controls share one row. */
function buildControls(toCompact: () => void, toHidden: () => void): HTMLElement {
  const controls = document.createElement("div");
  controls.className = "reader-sidebar__controls";

  const compactBtn = document.createElement("button");
  compactBtn.className = "reader-sidebar__hide";
  compactBtn.type = "button";
  compactBtn.title = "Collapse to a rail";
  compactBtn.setAttribute("aria-label", "Collapse to a rail");
  compactBtn.append(icon("reader-sidebar__hide-ic", '<path d="m11 17-5-5 5-5 M18 17l-5-5 5-5"></path>'));
  compactBtn.addEventListener("click", toCompact);

  const hideBtn = document.createElement("button");
  hideBtn.className = "reader-sidebar__hide";
  hideBtn.type = "button";
  hideBtn.title = "Hide the sidebar";
  hideBtn.setAttribute("aria-label", "Hide the sidebar");
  hideBtn.append(icon("reader-sidebar__hide-ic", '<rect width="18" height="18" x="3" y="3" rx="2"></rect><path d="M9 3v18 M16 15l-3-3 3-3"></path>'));
  hideBtn.addEventListener("click", toHidden);

  controls.append(compactBtn, hideBtn);
  return controls;
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

  const controls = buildControls(
    () => set("compact"),
    () => set("hidden"),
  );
  // The toprow is SSR (it carries "Back to Main") — drop the collapse controls into it so they
  // share the row; only prepend a bare row if an older page shipped without one.
  const toprow = inner.querySelector(".reader-sidebar__toprow");
  if (toprow) toprow.append(controls);
  else inner.prepend(controls);
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
// ON-THIS-PAGE OUTLINE + PROGRESS, fed by one scroll pass
// ─────────────────────────────────────────────────────────────────────────────

/** A rendered outline row — one per heading, in the desktop rail. */
interface OutlineRow {
  id: string;
  el: HTMLElement;
}

function wireOutline(headings: Heading[], onProgress: (fraction: number) => void): void {
  // ── the thin reading-progress bar across the very top (visible at every width) ──
  const bar = document.createElement("div");
  bar.className = "reader-progress";
  document.body.append(bar);

  // ── the scroll-to-top FAB (ships hidden; the CSS reveals it once we clear the fold) ──
  const toTop = document.createElement("button");
  toTop.className = "reader-scrolltop reader-scrolltop--hidden";
  toTop.type = "button";
  toTop.setAttribute("aria-label", "Scroll to top");
  toTop.append(icon("reader-scrolltop__ic", '<path d="m5 12 7-7 7 7"></path><path d="M12 19V5"></path>'));
  toTop.addEventListener("click", () => window.scrollTo({ top: 0, behavior: "smooth" }));
  document.body.append(toTop);

  // The desktop rail's rows; one scroll pass keeps the current section highlighted. Below 1180px
  // the rail is hidden and the reader menu's scrubber is how a reader moves between sections.
  const railRows: OutlineRow[] = [];
  let activeId: string | null = null;
  const reflectActive = (): void => {
    for (const { id, el } of railRows) el.classList.toggle("reader-outline__row--active", id === activeId);
  };

  if (headings.length > 0) {
    // ── the desktop rail outline, into the SSR nav.reader-outline ──
    const outline = document.querySelector(".reader-outline");
    if (outline) {
      const eyebrow = document.createElement("div");
      eyebrow.className = "reader-aside__eyebrow";
      eyebrow.textContent = "On this page";
      const list = document.createElement("ul");
      list.className = "reader-outline__list";
      for (const h of headings) {
        const li = document.createElement("li");
        li.className = "reader-outline__row";
        const a = document.createElement("a");
        a.href = `#${h.id}`;
        a.className = h.level >= 3 ? "reader-outline__btn reader-outline__btn--l3" : "reader-outline__btn";
        a.addEventListener("click", (event) => {
          event.preventDefault();
          scrollToHeading(h.id);
        });
        const tick = document.createElement("span");
        tick.className = "reader-outline__tick";
        const lbl = document.createElement("span");
        lbl.className = "reader-outline__label";
        lbl.textContent = h.text;
        a.append(tick, lbl);
        li.append(a);
        list.append(li);
        railRows.push({ id: h.id, el: li });
      }
      outline.append(eyebrow, list);
      log.debug(`chrome: rail outline ${headings.length} rows`);
    }
  }

  // ── one scroll pass drives the progress bar, the scroll-top FAB, and the active heading ──
  const recompute = (): void => {
    const track = document.documentElement.scrollHeight - window.innerHeight;
    const scroll = window.scrollY;
    const fraction = track > 0 ? Math.min(Math.max(scroll / track, 0), 1) : 0;
    bar.style.width = `${(fraction * 100).toFixed(2)}%`;
    toTop.classList.toggle("reader-scrolltop--hidden", scroll <= 400);
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

  const body = document.querySelector(".lesson-body");
  const headings = body ? harvestHeadings(body) : [];
  log.debug(`chrome: harvested ${headings.length} headings`);
  wireOutline(headings, faces.setRailProgress);
  wireMenu(headings);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
