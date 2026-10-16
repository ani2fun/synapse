import * as log from "../../lib/log";
// The reader menu's "Themes & Settings" view: the light/dark theme plus the four reading
// preferences, each a segmented control. The pure parse/serialize/applyToHtml half of the
// preferences lives in `lib/catalog/prefs.ts`, and the theme in `lib/theme.ts`; this file is only
// the editing UI, built when the view opens and torn down when it closes.

import * as storage from "../../lib/storage";
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
} from "../../lib/catalog/prefs";
import { isDark, setTheme } from "../../lib/theme";
import { THEME_CHANGED } from "../workbench/contracts";
import { icon } from "./headings";

const THEMES: readonly [string, string][] = [
  ["light", "Light"],
  ["dark", "Dark"],
];

/** A segmented control over one small allow-list. `preview` renders each option in the face its
 *  token names (`--serif/--sans/--mono`). `current` is read on every refresh, so the control
 *  always shows the live value rather than the one it was built with. */
function segmented(
  label: string,
  options: readonly [string, string][],
  preview: boolean,
  current: () => string,
  choose: (token: string) => void,
): { group: HTMLElement; refresh: () => void } {
  const group = document.createElement("div");
  group.className = "reader-settings__group";
  const labelEl = document.createElement("div");
  labelEl.className = "reader-settings__label";
  labelEl.textContent = label;
  const seg = document.createElement("div");
  seg.className = "reader-settings__seg";
  seg.setAttribute("role", "group");
  seg.setAttribute("aria-label", label);

  const buttons: { token: string; el: HTMLButtonElement }[] = [];
  for (const [token, display] of options) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = preview ? `reader-settings__opt reader-settings__opt--${token}` : "reader-settings__opt";
    btn.dataset.token = token;
    btn.textContent = display;
    btn.addEventListener("click", () => choose(token));
    seg.append(btn);
    buttons.push({ token, el: btn });
  }
  group.append(labelEl, seg);

  const refresh = (): void => {
    const now = current();
    for (const { token, el } of buttons) {
      const on = token === now;
      el.classList.toggle("reader-settings__opt--active", on);
      el.setAttribute("aria-pressed", String(on));
    }
  };
  return { group, refresh };
}

export interface SettingsView {
  el: HTMLElement;
  /** The back control, for focus on open. */
  back: HTMLButtonElement;
  destroy: () => void;
}

/** Build the settings view. `onBack` returns to the menu's root view. */
export function buildSettings(onBack: () => void): SettingsView {
  let prefs: Prefs = parsePrefs(storage.get(storage.READER_PREFS_KEY));
  const groups: { refresh: () => void }[] = [];
  const refreshAll = (): void => {
    for (const g of groups) g.refresh();
  };
  const commit = (next: Prefs): void => {
    prefs = next;
    applyToHtml(next);
    storage.set(storage.READER_PREFS_KEY, serializePrefs(next));
    refreshAll();
  };

  const el = document.createElement("div");
  el.className = "reader-settings";

  const head = document.createElement("div");
  head.className = "reader-settings__head";
  const back = document.createElement("button");
  back.type = "button";
  back.className = "reader-settings__back";
  back.setAttribute("aria-label", "Back to the reader menu");
  back.append(icon("reader-settings__back-ic", '<path d="m15 18-6-6 6-6"></path>'));
  back.addEventListener("click", onBack);
  const title = document.createElement("span");
  title.className = "reader-settings__title";
  title.textContent = "Themes & Settings";
  head.append(back, title);
  el.append(head);

  const theme = segmented(
    "Theme",
    THEMES,
    false,
    () => (isDark() ? "dark" : "light"),
    (token) => {
      setTheme(token === "dark");
      log.debug(`reader menu: theme → ${token}`);
    },
  );
  groups.push(theme);
  el.append(theme.group);

  const specs: [string, readonly [string, string][], keyof Prefs, boolean][] = [
    ["Size", SIZES, "size", false],
    ["Leading", LEADINGS, "leading", false],
    ["Type family", FAMILIES, "family", true],
    ["Width", WIDTHS, "width", false],
  ];
  for (const [label, options, field, preview] of specs) {
    const control = segmented(
      label,
      options,
      preview,
      () => prefs[field],
      (token) => {
        commit({ ...prefs, [field]: token });
        log.debug(`reader menu: prefs ${field} → ${token}`);
      },
    );
    groups.push(control);
    el.append(control.group);
  }

  const reset = document.createElement("button");
  reset.className = "reader-settings__reset";
  reset.type = "button";
  reset.textContent = "Reset to defaults";
  reset.addEventListener("click", () => {
    commit(DEFAULT_PREFS);
    log.info("reader menu: prefs reset to defaults");
  });
  el.append(reset);
  refreshAll();

  // The header's own toggle can flip the theme while this view is open.
  window.addEventListener(THEME_CHANGED, refreshAll);
  return { el, back, destroy: () => window.removeEventListener(THEME_CHANGED, refreshAll) };
}
