import * as log from "../lib/log";
import { isDark, setTheme } from "../lib/theme";
import { THEME_CHANGED } from "./workbench/contracts";

// The header's light/dark toggle. The `.dark` class on <html> is set pre-paint by Base.astro's
// inline bootstrap (a stored choice wins, else the OS preference); this island lets the reader flip
// it through `lib/theme`, which persists the result and fires THEME_CHANGED. The button re-reads
// the theme on that event, so a flip from the reader menu's settings keeps it truthful too. The
// Monaco editors observe <html>.dark through a MutationObserver, so they recolor on their own —
// nothing here reaches into them.

/** Keep the button's accessible state in step with the theme. The sun/moon glyph swap is pure CSS
 *  (`.dark .header__icon-btn__*` in shell.css); the label announces the action, not the state. */
function reflect(btn: Element, dark: boolean): void {
  btn.setAttribute("aria-pressed", String(dark));
  btn.setAttribute("aria-label", dark ? "Switch to light theme" : "Switch to dark theme");
}

function init(): void {
  const btn = document.querySelector("[data-theme-toggle]");
  if (!btn) return;
  reflect(btn, isDark());
  btn.addEventListener("click", () => {
    const dark = !isDark();
    setTheme(dark);
    log.info(`theme: switched to ${dark ? "dark" : "light"}`);
  });
  window.addEventListener(THEME_CHANGED, () => reflect(btn, isDark()));
  log.info("theme toggle: wired");
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
