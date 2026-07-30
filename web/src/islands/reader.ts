import * as log from "../lib/log";
// The reader's post-hydration chrome: done-ticks on the sidebar, reading-progress WRITES, the
// book-progress indicators (rail card + sidebar bar), the mobile nav drawer, and reflecting saved
// reading-preferences onto `<html>`. Vanilla TS, same
// reasoning as `islands/library.ts` — the SSR page is plain HTML and every job here is either
// `localStorage` (no SSR equivalent) or a scroll/click listener, so there is nothing for a
// component framework to hydrate INTO. This island loads on EVERY lesson page, including problem
// pages — which render no "Mark as read" switch, because the markup lives in the lesson branch of
// the page template, so the exclusion is structural rather than a runtime guard.
//
// Built on the pure `progress.ts`/`prefs.ts` helpers:
//   - done-ticks + the `--active`/`--done` classes (the exact class list, `.reader-sidebar__tick`
//     span, `aria-label="Finished"`).
//   - book-progress painting: the rail card + the sidebar bar, counted from the sidebar's links
//     and the done-set, re-painted after each server sync.
//   - progress WRITES (`reader-last`, `reader-progress`): idempotent both ways — a re-mark of an
//     already-finished lesson writes nothing, and so does un-marking one that was never marked —
//     driven by the end-of-page "Mark as read" switch. The reader decides; nothing marks a page
//     on their behalf.
//   - the mobile drawer (FAB → scrim + drawer, closes on scrim/Escape/any nav-link tap via
//     `closest("a")`).
//   - prefs: the `applyToHtml` half only — the FAB editing UI itself lives in `islands/chrome.ts`.
//
// The Compact rail, the on-this-page outline (desktop rail + mobile sheet), the top progress bar,
// the scroll-top FAB, and the reading-preferences FAB's editing UI live in `islands/chrome.ts`
// (lesson pages only, not problem pages). Not implemented
// anywhere yet: focus mode, the sidebar filter box, and the Learn-browse toggle — none of the
// e2e specs exercise them, and the SSR sidebar (`Sidebar.astro`/`SidebarTree.astro`) never
// renders their markup, so there is nothing half-wired left inert.

import * as storage from "../lib/storage";
import * as progress from "../lib/catalog/progress";
import * as api from "../lib/api/client";
import { AUTH_CHANGED, isAuthed } from "./workbench/contracts";
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
// DONE-TICKS
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

/** `markLinkDone`'s inverse, for one lesson: drop the class and the ✓ span from every sidebar
 *  copy of that link — the desktop sidebar AND any drawer clone, hence `querySelectorAll` rather
 *  than a single lookup. */
function removeDoneTick(root: ParentNode, path: string): void {
  root.querySelectorAll<HTMLAnchorElement>(".reader-sidebar__link--done").forEach((link) => {
    if (lessonPathFromHref(link.getAttribute("href") ?? "") !== path) return;
    link.classList.remove("reader-sidebar__link--done");
    link.querySelector(".reader-sidebar__tick")?.remove();
  });
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

/** Paint the book-progress indicators — the right-rail card and the sidebar bar — from the
 *  done-set. The sidebar shows only the current book, so counting its links is an exact "N of M".
 *  Re-run after each server sync (and each fresh tick) so the % follows the account, not just the
 *  device. A no-op where neither indicator is present (e.g. the sidebar-less problem page). */
function paintProgress(done: Set<string>): void {
  const links = Array.from(
    document.querySelectorAll<HTMLAnchorElement>(".reader-sidebar .reader-sidebar__link"),
  );
  const total = links.length;
  if (total === 0) return;
  let complete = 0;
  for (const link of links) {
    const path = lessonPathFromHref(link.getAttribute("href") ?? "");
    if (path && done.has(path)) complete += 1;
  }
  const pct = Math.round((complete / total) * 100);

  // The right-rail progress card.
  const pctEl = document.querySelector("[data-progress-pct]");
  if (pctEl) pctEl.textContent = `${pct}%`;
  const subEl = document.querySelector("[data-progress-sub]");
  if (subEl) subEl.textContent = `${complete}/${total} lessons`;
  const fillEl = document.querySelector<HTMLElement>("[data-progress-fill]");
  if (fillEl) fillEl.style.setProperty("--pct", `${pct}%`);

  // The sidebar book-progress bar (rides into the mobile Contents drawer via the sidebar clone).
  document.querySelectorAll<HTMLElement>("[data-book-progress]").forEach((box) => {
    box.hidden = false;
    box.querySelector<HTMLElement>("[data-book-progress-fill]")?.style.setProperty("--pct", `${pct}%`);
    const lbl = box.querySelector("[data-book-progress-sub]");
    if (lbl) lbl.textContent = `${complete} of ${total} lessons complete`;
  });

  log.debug(`reader: progress ${complete}/${total} (${pct}%)`);
}

// ─────────────────────────────────────────────────────────────────────────────
// PROGRESS WRITES
// ─────────────────────────────────────────────────────────────────────────────

/** Visit semantics: skip the write when the last-opened lesson hasn't changed. */
function visit(path: string): void {
  if (storage.get(storage.READER_LAST_KEY) === path) return;
  log.debug(`reader-last → ${path}`);
  storage.set(storage.READER_LAST_KEY, path);
}

/** Idempotent: re-marking an already-finished lesson writes nothing. */
function markDone(path: string): void {
  const done = readDone();
  if (done.has(path)) return;
  log.info(`lesson finished → reader-progress (${path})`);
  done.add(path);
  storage.set(storage.READER_PROGRESS_KEY, progress.serialize(done));
  // The just-finished lesson's own sidebar row gets its tick immediately, not only after the
  // next reload — matches marking-and-reading the same reactive set in one breath.
  applyDoneTicks(document, done);
  paintProgress(done);
  // A signed-in reader's progress is the ACCOUNT's, not the device's — persist it so the tick
  // survives a cache wipe and follows them to another browser. Anonymous stays localStorage-only.
  if (isAuthed()) void api.markProgress(path);
}

/** `markDone`'s inverse, idempotent the same way.
 *
 *  The server call is NOT optional for a signed-in reader: `syncFromServer` merges the account's
 *  completed set down into the local one, so a tick removed only in localStorage comes straight
 *  back on the next sync and the toggle appears to flip itself on. */
function unmarkDone(path: string): void {
  const done = readDone();
  if (!done.delete(path)) return;
  log.info(`lesson un-marked → reader-progress (${path})`);
  storage.set(storage.READER_PROGRESS_KEY, progress.serialize(done));
  removeDoneTick(document, path);
  paintProgress(done);
  if (isAuthed()) void api.unmarkProgress(path);
}

/** Reconcile the local ✓ set with the server for a signed-in reader: pull the account's completed
 *  paths down (mutating `done` IN PLACE so the nav-drawer closure that captured it still shows the
 *  new ticks), then push this browser's pre-sign-in ticks up. The push list drains to empty once
 *  the server has them, so later syncs only download. Anonymous callers never reach here. */
async function syncFromServer(done: Set<string>): Promise<void> {
  try {
    const server = await api.listProgress();
    let added = false;
    for (const path of server) {
      if (!done.has(path)) {
        done.add(path);
        added = true;
      }
    }
    if (added) {
      storage.set(storage.READER_PROGRESS_KEY, progress.serialize(done));
      applyDoneTicks(document, done);
      paintProgress(done);
    }
    const onServer = new Set(server);
    const localOnly = [...done].filter((path) => !onServer.has(path));
    for (const path of localOnly) void api.markProgress(path);
    log.info(`progress synced — ${server.length} down, ${localOnly.length} up`);
  } catch (error) {
    log.debug(`progress sync skipped: ${error instanceof Error ? error.message : String(error)}`);
  }
}

/** The end-of-page "Mark as read" switch: the reader's own decision, replacing the scroll
 *  threshold that used to make it for them.
 *
 *  Rendered unchecked by SSR because the server cannot know — the ✓ set lives in localStorage —
 *  so the first job here is to show the truth. `paint` is also the re-sync hook: a signed-in
 *  reader's account state lands after `syncFromServer`, and the switch has to follow it. */
function wireReadToggle(path: string): () => void {
  const button = document.querySelector<HTMLButtonElement>("[data-read-switch]");
  if (!button) return () => {};
  const paint = (): void => {
    button.setAttribute("aria-checked", readDone().has(path) ? "true" : "false");
  };
  button.addEventListener("click", () => {
    // Read from storage rather than the attribute: the attribute is what the reader SEES, and a
    // sync landing mid-click would make it the stale half of the pair.
    if (readDone().has(path)) unmarkDone(path);
    else markDone(path);
    paint();
  });
  paint();
  return paint;
}

// ─────────────────────────────────────────────────────────────────────────────
// THE MOBILE NAV DRAWER
// ─────────────────────────────────────────────────────────────────────────────

// The FAB is the LESSON page's trigger; the PROBLEM page has no FAB — its docked `.pwb__nav`
// Contents pill fires the `OPEN_CONTENTS` window event instead. So the drawer is set
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
// TWO-MODE SIDEBAR (book contents ⇄ all-books browse)
// ─────────────────────────────────────────────────────────────────────────────

/** "Back to Main" flips the Expanded face from the current book's contents to the library browse
 *  list (all books); the browse view's own back button flips it back. Delegated on `document`, so
 *  one handler drives the desktop sidebar AND the cloned drawer (a clone carries no listeners). The
 *  trigger is an `<a href="/">`, so without JS it still falls back to the library page. */
function wireSidebarViewToggle(): void {
  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const toggle = target.closest<HTMLElement>("[data-sidebar-view]");
    if (!toggle) return;
    const inner = toggle.closest<HTMLElement>(".reader-sidebar__inner");
    if (!inner) return;
    event.preventDefault();
    const view = toggle.getAttribute("data-sidebar-view") ?? "book";
    inner.setAttribute("data-view", view);
    log.debug(`reader: sidebar view → ${view}`);
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// PREFS (`applyToHtml` half only)
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
  paintProgress(done);
  wireNavDrawer(done);
  wireSidebarViewToggle();

  // Wired before the first sync so the switch shows local truth immediately; `repaintToggle` is
  // how the ACCOUNT's answer reaches it once that lands. A problem page renders no switch — the
  // markup is inside the lesson branch — so this is a no-op there, and a problem still earns its
  // tick from an accepted submission rather than from a control nobody sees.
  const repaintToggle = path ? wireReadToggle(path) : () => {};

  // Signed-in readers reconcile with the account's progress — now, and again whenever auth adopts
  // late (the store fetches its config async, so `isAuthed()` is often false on first paint).
  const sync = (): void => {
    if (isAuthed()) void syncFromServer(done).then(repaintToggle);
  };
  sync();
  window.addEventListener(AUTH_CHANGED, sync);

  if (path) visit(path);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
