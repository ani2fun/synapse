import * as log from "../../lib/log";
// The reader menu — one round button at the bottom-right of a lesson that opens everything the
// reader steers by: Browse (the book's lessons), Search Book (the palette, confined to this
// book) and Themes & Settings (light/dark and the reading preferences). Opening it also raises the
// vertical scrubber down the right edge, for moving through THIS page; closing it takes both away.
//
// The rows reach other islands by event — `OPEN_CONTENTS` to the drawer in `islands/reader.ts`,
// `OPEN_SEARCH` to the palette — so this file owns no drawer and no search. The book a search is
// confined to comes from `data-book-slug`/`data-book-title` on the page, read when the row is
// chosen: a private lesson sets them only once its payload has arrived.

import { OPEN_CONTENTS, OPEN_SEARCH, type OpenSearch, type SearchScope } from "../workbench/contracts";
import { icon, type Heading } from "./headings";
import { mountScrubber, type Scrubber } from "./scrubber";
import { buildSettings, type SettingsView } from "./settings";

/** Two bars over three dots. */
const MENU_ICON =
  '<path d="M5 7h14 M5 11.5h14"></path>' +
  '<circle cx="7" cy="17" r="1.4" fill="currentColor" stroke="none"></circle>' +
  '<circle cx="12" cy="17" r="1.4" fill="currentColor" stroke="none"></circle>' +
  '<circle cx="17" cy="17" r="1.4" fill="currentColor" stroke="none"></circle>';
/** The library glyph (books on a shelf) — the one the desktop Browse pill and the problem page's
 *  pill wear, because this row opens the same drawer and so says the same thing. */
const BROWSE_ICON = '<path d="m16 6 4 14 M12 6v14 M8 8v12 M4 4v16"></path>';
const SEARCH_ICON = '<circle cx="11" cy="11" r="6.5"></circle><path d="m20 20-4.2-4.2"></path>';

/** The book this lesson belongs to, as the page names it — or null when it names none. */
function pageBook(): SearchScope | null {
  const el = document.querySelector<HTMLElement>("[data-book-slug]");
  const slug = el?.dataset.bookSlug ?? "";
  if (slug === "") return null;
  return { slug, title: el?.dataset.bookTitle || slug };
}

/** One pill row: the label on the left, its glyph on the right. */
function row(label: string, glyph: Element, choose: () => void): HTMLButtonElement {
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "reader-menu__row";
  btn.setAttribute("role", "menuitem");
  const text = document.createElement("span");
  text.className = "reader-menu__label";
  text.textContent = label;
  btn.append(text, glyph);
  btn.addEventListener("click", choose);
  return btn;
}

/** The "AA" glyph of the settings row — a small A beside a large one, as type. */
function typeGlyph(): HTMLElement {
  const glyph = document.createElement("span");
  glyph.className = "reader-menu__aa";
  glyph.setAttribute("aria-hidden", "true");
  const small = document.createElement("span");
  small.className = "reader-menu__aa-small";
  small.textContent = "A";
  glyph.append(small, "A");
  return glyph;
}

export function wireMenu(headings: Heading[]): void {
  const host = document.createElement("div");
  host.className = "reader-menu-host";

  const fab = document.createElement("button");
  fab.className = "reader-menu-fab";
  fab.type = "button";
  fab.title = "Reader menu";
  fab.setAttribute("aria-label", "Reader menu");
  fab.setAttribute("aria-haspopup", "menu");
  fab.setAttribute("aria-expanded", "false");
  fab.append(icon("reader-menu-fab__ic", MENU_ICON));

  // Built on open and removed on close, never toggled with `hidden`: the class rules carry a
  // `display`, which outranks `[hidden]`'s UA `display: none`.
  let scrim: HTMLDivElement | null = null;
  let pop: HTMLDivElement | null = null;
  let scrubber: Scrubber | null = null;
  let settings: SettingsView | null = null;
  let rows: HTMLButtonElement[] = [];

  /** Close the menu. `refocus` returns focus to the button — wanted when the reader dismissed
   *  it, not when a row hands focus on to the drawer or the palette. */
  const close = (refocus: boolean): void => {
    if (!pop) return;
    settings?.destroy();
    scrubber?.destroy();
    scrim?.remove();
    pop.remove();
    scrim = null;
    pop = null;
    scrubber = null;
    settings = null;
    rows = [];
    fab.setAttribute("aria-expanded", "false");
    document.body.classList.remove("reader-menu-open");
    if (refocus) fab.focus({ preventScroll: true });
    log.info("reader menu: closed");
  };

  const showRoot = (focusIndex: number): void => {
    if (!pop) return;
    settings?.destroy();
    settings = null;
    const menu = document.createElement("div");
    menu.className = "reader-menu";
    menu.setAttribute("role", "menu");
    menu.setAttribute("aria-label", "Reader menu");
    rows = [
      row("Browse", icon("reader-menu__ic", BROWSE_ICON), () => {
        log.debug("reader menu: browse");
        close(false);
        window.dispatchEvent(new Event(OPEN_CONTENTS));
      }),
      row("Search Book", icon("reader-menu__ic", SEARCH_ICON), () => {
        const book = pageBook();
        log.debug(`reader menu: search ${book ? `book ${book.slug}` : "every book (the page names none)"}`);
        close(false);
        const detail: OpenSearch = book ? { book } : {};
        window.dispatchEvent(new CustomEvent<OpenSearch>(OPEN_SEARCH, { detail }));
      }),
      row("Themes & Settings", typeGlyph(), () => {
        log.debug("reader menu: settings");
        showSettings();
      }),
    ];
    menu.append(...rows);
    menu.addEventListener("keydown", (event) => {
      const at = rows.indexOf(document.activeElement as HTMLButtonElement);
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const step = event.key === "ArrowDown" ? 1 : -1;
        rows[(at + step + rows.length) % rows.length]?.focus();
      } else if (event.key === "Home" || event.key === "End") {
        event.preventDefault();
        rows[event.key === "Home" ? 0 : rows.length - 1]?.focus();
      }
    });
    pop.replaceChildren(menu);
    rows[focusIndex]?.focus({ preventScroll: true });
  };

  const showSettings = (): void => {
    if (!pop) return;
    rows = [];
    settings = buildSettings(() => showRoot(2));
    pop.replaceChildren(settings.el);
    settings.back.focus({ preventScroll: true });
  };

  const open = (): void => {
    if (pop) return;
    scrim = document.createElement("div");
    scrim.className = "reader-menu-scrim";
    scrim.addEventListener("click", () => close(true));
    pop = document.createElement("div");
    pop.className = "reader-menu-pop";
    host.append(scrim, pop);
    scrubber = mountScrubber(host, headings);
    fab.setAttribute("aria-expanded", "true");
    document.body.classList.add("reader-menu-open");
    showRoot(0);
    log.info("reader menu: open");
  };

  fab.addEventListener("click", () => (pop ? close(true) : open()));
  window.addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || !pop) return;
    // One step back at a time: the settings view returns to the menu before the menu closes.
    if (settings) showRoot(2);
    else close(true);
  });

  host.append(fab);
  document.body.append(host);
}
