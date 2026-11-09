// Reading-preferences tokens — the pure half. Five independent choices, each a small allow-list;
// persisted as one `|`-joined string. Unknown tokens degrade per-field to the default (a bad
// stored value must never break the reader).
//
// This module is otherwise pure — no `localStorage`, no DOM — except for `applyToHtml`, folded
// in here since the reflect-onto-`<html>` step is cheap enough not to need a separate file. The
// editing UI that lets a reader change these is the reader menu's settings view
// (`islands/reader/settings.ts`).

export interface Prefs {
  size: string;
  leading: string;
  family: string;
  width: string;
  /** Whether an in-page jump (an outline row, a `#` link, scroll-to-top) animates. */
  scroll: string;
}

export const DEFAULT_PREFS: Prefs = {
  size: "md",
  leading: "normal",
  family: "sans",
  width: "standard",
  scroll: "instant",
};

export const SIZES: [string, string][] = [
  ["sm", "Small"],
  ["md", "Medium"],
  ["lg", "Large"],
];
export const LEADINGS: [string, string][] = [
  ["tight", "Tight"],
  ["normal", "Comfortable"],
  ["loose", "Loose"],
];
export const FAMILIES: [string, string][] = [
  ["serif", "Serif"],
  ["sans", "Sans"],
  ["mono", "Mono"],
];
export const WIDTHS: [string, string][] = [
  ["narrow", "Narrow"],
  ["standard", "Standard"],
  ["wide", "Wide"],
];
/** Instant is the default: a reader jumping between sections wants to be there, not to watch
 *  the page travel. */
export const SCROLLS: [string, string][] = [
  ["instant", "Instant"],
  ["smooth", "Smooth"],
];

function canonical(options: [string, string][], token: string, fallback: string): string {
  const found = options.find(([t]) => t === token);
  return found ? found[0] : fallback;
}

/** Parse a stored `size|leading|family|width|scroll` string; anything malformed degrades per
 *  field. */
export function parse(stored: string | null | undefined): Prefs {
  if (stored === null || stored === undefined) return DEFAULT_PREFS;
  const parts = stored.split("|");
  // A 4-part `size|leading|family|width` string is what a reader saved before Scrolling existed:
  // it keeps its four choices and takes the default scroll.
  if (parts.length !== 4 && parts.length !== 5) return DEFAULT_PREFS;
  const [s, l, f, w, sc = DEFAULT_PREFS.scroll] = parts;
  return {
    size: canonical(SIZES, s, DEFAULT_PREFS.size),
    leading: canonical(LEADINGS, l, DEFAULT_PREFS.leading),
    family: canonical(FAMILIES, f, DEFAULT_PREFS.family),
    width: canonical(WIDTHS, w, DEFAULT_PREFS.width),
    scroll: canonical(SCROLLS, sc, DEFAULT_PREFS.scroll),
  };
}

export function serialize(prefs: Prefs): string {
  return `${prefs.size}|${prefs.leading}|${prefs.family}|${prefs.width}|${prefs.scroll}`;
}

/** Reflect the five choices onto `<html data-reader-*>` — the stylesheet reads these attributes
 *  (`reader.css`'s `html[data-reader-size="…"] .synapse-prose`, etc). Set once on load, before
 *  the FAB itself mounts — a saved preference must apply even before the editing UI is ready. */
export function applyToHtml(prefs: Prefs, root: HTMLElement = document.documentElement): void {
  root.setAttribute("data-reader-size", prefs.size);
  root.setAttribute("data-reader-leading", prefs.leading);
  root.setAttribute("data-reader-family", prefs.family);
  root.setAttribute("data-reader-width", prefs.width);
  root.setAttribute("data-reader-scroll", prefs.scroll);
}
