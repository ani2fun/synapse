// Reading-preferences tokens — the pure half. Four independent choices, each a small allow-list;
// persisted as one `|`-joined string. Unknown tokens degrade per-field to the default (a bad
// stored value must never break the reader).
//
// This module is otherwise pure — no `localStorage`, no DOM — except for `applyToHtml`, folded
// in here since the reflect-onto-`<html>` step is cheap enough not to need a separate file. The
// FAB *editing* UI that lets a reader change these lives in `islands/chrome.ts`.

export interface Prefs {
  size: string;
  leading: string;
  family: string;
  width: string;
}

export const DEFAULT_PREFS: Prefs = {
  size: "md",
  leading: "normal",
  family: "sans",
  width: "standard",
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

function canonical(options: [string, string][], token: string, fallback: string): string {
  const found = options.find(([t]) => t === token);
  return found ? found[0] : fallback;
}

/** Parse a stored `size|leading|family|width` string; anything malformed degrades per field. */
export function parse(stored: string | null | undefined): Prefs {
  if (stored === null || stored === undefined) return DEFAULT_PREFS;
  const parts = stored.split("|");
  if (parts.length !== 4) return DEFAULT_PREFS;
  const [s, l, f, w] = parts;
  return {
    size: canonical(SIZES, s, DEFAULT_PREFS.size),
    leading: canonical(LEADINGS, l, DEFAULT_PREFS.leading),
    family: canonical(FAMILIES, f, DEFAULT_PREFS.family),
    width: canonical(WIDTHS, w, DEFAULT_PREFS.width),
  };
}

export function serialize(prefs: Prefs): string {
  return `${prefs.size}|${prefs.leading}|${prefs.family}|${prefs.width}`;
}

/** Reflect the four choices onto `<html data-reader-*>` — the stylesheet reads these attributes
 *  (`reader.css`'s `html[data-reader-size="…"] .synapse-prose`, etc). Set once on load, before
 *  the FAB itself mounts — a saved preference must apply even before the editing UI is ready. */
export function applyToHtml(prefs: Prefs, root: HTMLElement = document.documentElement): void {
  root.setAttribute("data-reader-size", prefs.size);
  root.setAttribute("data-reader-leading", prefs.leading);
  root.setAttribute("data-reader-family", prefs.family);
  root.setAttribute("data-reader-width", prefs.width);
}
