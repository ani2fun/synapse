import * as log from "../lib/log";
// The reader's post-hydration chrome (A05): done-ticks on the sidebar, reading-progress WRITES,
// the mobile nav drawer, and reflecting saved reading-preferences onto `<html>`. Vanilla TS, same
// reasoning as `islands/library.ts` — the SSR page is plain HTML and every job here is either
// `localStorage` (no SSR equivalent) or a scroll/click listener, so there is nothing for a
// component framework to hydrate INTO.
//
// Oracle spec, ported through A04's pure `progress.ts`/`prefs.ts` rather than re-derived:
//   - done-ticks + the `--active`/`--done` classes: client/src/catalog/view/sidebar.rs's
//     `lesson_link` (the exact class list, `.reader-sidebar__tick` span, `aria-label="Finished"`).
//   - progress WRITES (`reader-last`, `reader-progress`): client/src/catalog/state/mod.rs's
//     `ProgressStore.visit`/`set_done` (idempotent — a re-mark of an already-finished lesson
//     writes nothing) driven by `catalog/view/chrome.rs`'s scroll recompute + `progress::is_at_end`.
//   - the mobile drawer: `catalog/view/reader.rs`'s `ReaderNavDrawer` (FAB → scrim + drawer,
//     closes on scrim/Escape/any nav-link tap via `closest("a")`).
//   - prefs: `catalog/state/mod.rs`'s `PrefsStore::provide` (`apply_to_html` half only — the FAB
//     EDITING panel is deferred, see the A05 chapter).
//
// Deferred (not this step's job — see the A05 chapter for the full list): the Compact rail, the
// minimap, the sticky bar, the TOC FAB, focus mode, the sidebar filter box, the Learn-browse
// toggle, and the reading-preferences FAB's editing UI. None of the seven e2e specs exercise any
// of them, and the SSR sidebar (`Sidebar.astro`/`SidebarTree.astro`) never rendered their markup
// in the first place — there is nothing half-wired to leave inert.

import * as storage from "../lib/storage";
import * as progress from "../lib/catalog/progress";
import { parse as parsePrefs, applyToHtml } from "../lib/catalog/prefs";

const SYNAPSE_PREFIX = "/synapse/";

function currentLessonPath(): string | null {
  const { pathname } = window.location;
  if (!pathname.startsWith(SYNAPSE_PREFIX)) return null;
  const path = decodeURIComponent(pathname.slice(SYNAPSE_PREFIX.length)).replace(/\/+$/, "");
  return path === "" ? null : path;
}

/** A sidebar link's own lesson path, read off its `href` — works for both the desktop sidebar
 *  and any clone of it (the mobile drawer), since both carry the same `/synapse/{path}` hrefs. */
function lessonPathFromHref(href: string): string | null {
  try {
    const url = new URL(href, window.location.origin);
    if (!url.pathname.startsWith(SYNAPSE_PREFIX)) return null;
    return decodeURIComponent(url.pathname.slice(SYNAPSE_PREFIX.length)).replace(/\/+$/, "");
  } catch {
    return null;
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// DONE-TICKS (oracle: sidebar.rs's `lesson_link`)
// ─────────────────────────────────────────────────────────────────────────────

function markLinkDone(link: HTMLAnchorElement): void {
  if (link.classList.contains("reader-sidebar__link--done")) return;
  link.classList.add("reader-sidebar__link--done");
  const tick = document.createElement("span");
  tick.className = "reader-sidebar__tick";
  tick.setAttribute("aria-label", "Finished");
  tick.textContent = "✓";
  link.append(tick);
}

/** Apply done-ticks to every sidebar link within `root` whose lesson is in the finished set —
 *  the desktop sidebar on load, and the SAME call re-run against a cloned drawer (a clone
 *  copies the classes/tick along with it, but re-running costs nothing and stays correct if
 *  the done set changed between the initial render and the drawer opening). */
function applyDoneTicks(root: ParentNode, done: Set<string>): void {
  root.querySelectorAll<HTMLAnchorElement>(".reader-sidebar__link").forEach((link) => {
    const path = lessonPathFromHref(link.getAttribute("href") ?? "");
    if (path && done.has(path)) markLinkDone(link);
  });
}

function readDone(): Set<string> {
  return progress.parse(storage.get(storage.READER_PROGRESS_KEY));
}

// ─────────────────────────────────────────────────────────────────────────────
// PROGRESS WRITES (oracle: `ProgressStore.visit`/`set_done`, `progress::is_at_end`)
// ─────────────────────────────────────────────────────────────────────────────

/** Visit semantics: skip the write when the last-opened lesson hasn't changed. */
function visit(path: string): void {
  if (storage.get(storage.READER_LAST_KEY) === path) return;
  log.debug(`reader-last → ${path}`);
  storage.set(storage.READER_LAST_KEY, path);
}

/** Idempotent: re-marking an already-finished lesson writes nothing (mirrors
 *  `ProgressStore.set_done`'s `changed == Some(true)` guard). */
function markDone(path: string): void {
  const done = readDone();
  if (done.has(path)) return;
  log.info(`lesson finished → reader-progress (${path})`);
  done.add(path);
  storage.set(storage.READER_PROGRESS_KEY, progress.serialize(done));
  // The just-finished lesson's own sidebar row gets its tick immediately, not only after the
  // next reload — matches marking-and-reading the same reactive set in one breath.
  applyDoneTicks(document, done);
}

function wireProgress(path: string): void {
  const recompute = (): void => {
    const track = document.documentElement.scrollHeight - window.innerHeight;
    const scroll = window.scrollY;
    if (progress.isAtEnd(scroll, track)) markDone(path);
  };
  recompute(); // a lesson shorter than the viewport is "read" the moment it paints
  window.addEventListener("scroll", recompute, { passive: true });
}

// ─────────────────────────────────────────────────────────────────────────────
// THE MOBILE NAV DRAWER (oracle: `ReaderNavDrawer`)
// ─────────────────────────────────────────────────────────────────────────────

// The FAB is the LESSON page's trigger; the PROBLEM page (A07) has no FAB — its docked
// `.pwb__nav` Contents pill fires the `OPEN_CONTENTS` window event instead. So the drawer is set
// up whenever there is a sidebar to clone, the FAB is optional, and the mount host is `.reader-nav`
// when present (the lesson layout) else `document.body` (the problem layout, which keeps the
// sidebar markup only as a hidden clone source). scrim/drawer are `position: fixed`, so the parent
// choice is presentational, not positional.
function wireNavDrawer(done: Set<string>): void {
  const nav = document.querySelector<HTMLElement>(".reader-nav");
  const fab = nav?.querySelector<HTMLButtonElement>(".reader-nav-fab") ?? null;
  const sidebarInner = document.querySelector<HTMLElement>(".reader-sidebar .reader-sidebar__inner");
  // Nothing to open onto: no lesson FAB AND no problem-page contents source.
  if (!fab && !sidebarInner) return;
  const host = nav ?? document.body;

  let scrim: HTMLDivElement | null = null;
  let drawer: HTMLElement | null = null;

  const close = (): void => {
    scrim?.remove();
    drawer?.remove();
    scrim = null;
    drawer = null;
    fab?.setAttribute("aria-expanded", "false");
  };

  const open = (): void => {
    if (drawer) return;
    log.debug("contents drawer opened");
    scrim = document.createElement("div");
    scrim.className = "reader-nav-scrim";
    scrim.addEventListener("click", close);

    drawer = document.createElement("aside");
    drawer.className = "reader-nav-drawer";
    drawer.addEventListener("click", (event) => {
      const target = event.target;
      if (target instanceof Element && target.closest("a")) close();
    });

    const head = document.createElement("div");
    head.className = "reader-nav-drawer__head";
    const title = document.createElement("span");
    title.className = "reader-nav-drawer__title";
    title.textContent = "Contents";
    const closeBtn = document.createElement("button");
    closeBtn.className = "reader-nav-drawer__close";
    closeBtn.setAttribute("aria-label", "Close");
    closeBtn.textContent = "✕";
    closeBtn.addEventListener("click", close);
    head.append(title, closeBtn);
    drawer.append(head);

    if (sidebarInner) {
      const clone = sidebarInner.cloneNode(true) as HTMLElement;
      applyDoneTicks(clone, done);
      drawer.append(clone);
    }

    host.append(scrim, drawer);
    fab?.setAttribute("aria-expanded", "true");
  };

  fab?.addEventListener("click", open);
  // The problem page's Contents pill lives in another island; it reaches this drawer by event.
  window.addEventListener("synapse:open-contents", open);
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && drawer) close();
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// PREFS (oracle: `PrefsStore::provide`'s `apply_to_html` half)
// ─────────────────────────────────────────────────────────────────────────────

function applyStoredPrefs(): void {
  const prefs = parsePrefs(storage.get(storage.READER_PREFS_KEY));
  applyToHtml(prefs);
}

function init(): void {
  applyStoredPrefs();

  const path = currentLessonPath();
  const done = readDone();
  applyDoneTicks(document, done);
  wireNavDrawer(done);

  if (path) {
    visit(path);
    // A problem page (A07) scrolls its PANES internally, not the window, so the page's own
    // scroll track is ~0 and `isAtEnd` would mark it done the moment it paints. The oracle's
    // problem layout has no ProgressStore scroll tracking at all — match it: `visit` still
    // records "last opened" (the library's resume card), but done-on-scroll stays off.
    if (!document.querySelector(".pwb[data-problem]")) wireProgress(path);
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
