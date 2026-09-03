// The one `localStorage` accessor. Every preference in the app persists through here: reader
// prefs, reading progress, the sidebar face, the workbench language, the problem-page panes, the
// theme.
//
// Both read and write swallow failure by design — Safari's private mode and a
// cookies-disabled profile both make `localStorage` throw rather than return `null`, and a
// preference that cannot be saved must never take the page down with it. The explicit
// `typeof window` check below covers Astro's server render, which has no `window` at all.
//
// Any new preference feature should add its own key here rather than reinventing the accessor.

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

/** Drop every key under a prefix. The inventory below holds two kinds of name: exact keys, which
 *  `remove` handles, and PREFIXES, whose real keys are minted at runtime and so cannot be listed
 *  ahead of time. The account page's "erase all my data" needs both. Enumeration and removal are
 *  guarded together — a profile that denies `localStorage` throws on `key()` just as it does on
 *  `getItem`. */
export function removeByPrefix(prefix: string): void {
  if (typeof window === "undefined") return;
  try {
    const doomed: string[] = [];
    for (let i = 0; i < window.localStorage.length; i += 1) {
      const key = window.localStorage.key(i);
      if (key !== null && key.startsWith(prefix)) doomed.push(key);
    }
    // Collected first: removing during the walk reindexes the store underneath it.
    for (const key of doomed) window.localStorage.removeItem(key);
  } catch {
    // swallow — see the module doc.
  }
}

// ── the key inventory ───────────────────────────────────────────────────────────────────────
// One name per feature, spelled once, so a typo in a second call site can't silently start a
// new key instead of colliding with a lint.

/** The four-field reading-preferences pack (size · leading · family · width). */
export const READER_PREFS_KEY = "reader-prefs";
/** The newline-set of finished lesson paths (`progress.ts`). */
export const READER_PROGRESS_KEY = "reader-progress";
/** The last lesson path opened — the library's "continue where you left off" card. */
export const READER_LAST_KEY = "reader-last";
/** The sidebar's persisted face: expanded / compact / hidden. */
export const READER_SIDEBAR_KEY = "reader-sidebar";
/** The problem workbench's two-pane split percentage. */
export const PROBLEM_PANE_KEY = "problem-pane";
/** The problem workbench's remembered editorial approach tab. */
export const PROBLEM_APPROACH_KEY = "problem-approach";
/** Which side of the workbench a problem OPENS on — `"think"` or `"code"`. Absent means Think,
 *  because the method the page is built around says the plan comes first.
 *
 *  Written only by the explicit pin, never by switching tabs: peeking at the editor is not the
 *  same statement as "this is how I want to start", and a preference a casual click can
 *  overwrite is not a preference. */
export const PROBLEM_MODE_KEY = "problem-mode";
/** The runnable block's remembered language tab. */
export const WB_LANGUAGE_KEY = "wb-language";
/** `"dark" | "light"` — read pre-paint by `Base.astro`'s inline bootstrap script too. */
export const THEME_KEY = "theme";
/** The diagram editors' two-pane split percentage. Shared by `/d2` and `/mermaid`: how wide the
 *  source pane sits is a preference about the workspace, not about the language in it. */
export const DIAGRAM_LAB_PANE_KEY = "diagram-lab-pane";
/** The diagram editors' in-progress diagram — a key PREFIX. The language is appended, and a
 *  diagram opened from a lesson appends its path and ordinal too (see `diagramlab/Lab`), so two
 *  languages and two diagrams never overwrite each other's autosave. No account is involved: a
 *  diagram only becomes anyone's when it is copied into a lesson or proposed as an edit. */
export const DIAGRAM_LAB_DRAFT_PREFIX = "diagram-lab-draft";
/** The content editor's per-page draft key PREFIX — the username and lesson path are appended
 *  (`content-draft:<username>:<lesson-path>`) so one browser can hold a draft for each page a
 *  contributor is editing, and a draft never leaks across accounts. See islands/authoring/draft. */
export const CONTENT_DRAFT_PREFIX = "content-draft:";
/** The popup codebench's edited buffer — a key PREFIX. The username, the page path, the language
 *  and a fingerprint of the authored fence are appended, so two snippets never overwrite each
 *  other's draft, a draft never surfaces under another account, and an edit to the lesson retires
 *  the draft that no longer applies. See islands/widgets/codebenchDraft. */
export const CODEBENCH_DRAFT_PREFIX = "codebench-draft:";
/** The design canvas's in-progress draft — a key PREFIX. The username and the problem path
 *  are appended (`canvas-draft:<username>:<problem-path>`), so a draft never surfaces under
 *  another account and two problems never overwrite each other. SAVED entries are not here:
 *  they are the account's, in Postgres, the way submissions are. See islands/canvas/draft. */
export const CANVAS_DRAFT_PREFIX = "canvas-draft:";
