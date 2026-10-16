// The light/dark theme, one place for every control that flips it. `.dark` on <html> IS the
// theme: Base.astro's inline bootstrap sets it before first paint (a stored choice wins, else the
// OS preference), Monaco observes it, and the stylesheets read it. Flipping it here also persists
// the choice under THEME_KEY and fires THEME_CHANGED, so the header toggle and the reader menu's
// settings never disagree about which theme is on.

import * as storage from "./storage";
import { THEME_CHANGED, type ThemeChanged } from "../islands/workbench/contracts";

/** Is the dark theme currently on? */
export function isDark(): boolean {
  return document.documentElement.classList.contains("dark");
}

/** Switch the theme, persist the choice, and tell every theme control. */
export function setTheme(dark: boolean): void {
  document.documentElement.classList.toggle("dark", dark);
  storage.set(storage.THEME_KEY, dark ? "dark" : "light");
  window.dispatchEvent(new CustomEvent<ThemeChanged>(THEME_CHANGED, { detail: { dark } }));
}
