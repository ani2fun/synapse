// The one `localStorage` accessor (oracle: client/src/storage.rs). Every preference in the app
// persists through here: reader prefs, reading progress, the sidebar face, the workbench
// language, the problem-page panes, the theme.
//
// Both read and write swallow failure by design — Safari's private mode and a
// cookies-disabled profile both make `localStorage` throw rather than return `null`, and a
// preference that cannot be saved must never take the page down with it. The Rust oracle gets
// this for free from `web_sys::window()?...ok()??` (an absent `window()` — no browser — reads
// the same as a denied storage call); the SSR equivalent is the explicit `typeof window`
// check below, because Astro's server render has no `window` at all.
//
// A05 (the mobile drawer's sidebar-face persistence, the theme toggle, the ⌘K palette's recent
// searches — whichever of those land there) is the next consumer of this module; nothing about
// it is A04-specific.

/** Read a key; absent, unreadable, storage-denied, or server-rendered (no `window`) all read
 *  as `null`. */
export function get(key: string): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Write a key; a denied write (or no `window`) is silently a no-op. */
export function set(key: string, value: string): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // swallow — see the module doc.
  }
}

/** Drop a key; a denied removal (or no `window`) is silently a no-op. Used by the account
 *  page's "erase all my data", which must be able to take reading progress with it. */
export function remove(key: string): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(key);
  } catch {
    // swallow — see the module doc.
  }
}

// ── the key inventory (oracle: every `const … _KEY` across client/src) ────────────────────────
// One name per feature, spelled once, so a typo in a second call site can't silently start a
// new key instead of colliding with a lint.

/** The four-field reading-preferences pack (size · leading · family · width). */
export const READER_PREFS_KEY = "reader-prefs";
/** The newline-set of finished lesson paths (this step's `progress.ts`). */
export const READER_PROGRESS_KEY = "reader-progress";
/** The last lesson path opened — the library's "continue where you left off" card. */
export const READER_LAST_KEY = "reader-last";
/** The sidebar's persisted face: expanded / compact / hidden. */
export const READER_SIDEBAR_KEY = "reader-sidebar";
/** The problem workbench's two-pane split percentage. */
export const PROBLEM_PANE_KEY = "problem-pane";
/** The problem workbench's remembered editorial approach tab. */
export const PROBLEM_APPROACH_KEY = "problem-approach";
/** The runnable block's remembered language tab. */
export const WB_LANGUAGE_KEY = "wb-language";
/** `"dark" | "light"` — read pre-paint by `Base.astro`'s inline bootstrap script too. */
export const THEME_KEY = "theme";
