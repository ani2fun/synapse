/**
 * The workbench's cross-island contracts. Islands cannot share signals, so every seam becomes a
 * named CustomEvent or a window-scoped provider — ALL of them declared here, once, because an
 * event name in two files is a typo waiting to disagree.
 */

/** Dispatched ON a workbench root by the editorial (copy-to-editor). detail: LoadCode. The event
 *  itself is the tick — re-dispatching the same code fires again by construction. */
export const LOAD_CODE = "synapse:load-code";
export interface LoadCode {
  language: string;
  code: string;
}

/** Dispatched ON a workbench root by the Submissions rows (reproduce a failing input).
 *  detail: UseCase — the TestsPanel appends and selects it. */
export const USE_CASE = "synapse:use-case";
export interface UseCase {
  args: Record<string, string>;
  expected: string | null;
}

/** Dispatched (bubbling) FROM a workbench root when a submit lifecycle completes — the
 *  Submissions tab refetches on it. */
export const SUBMITTED = "synapse:submitted";

/** Dispatched (bubbling) FROM a workbench root on every buffer edit / tab switch — the coach
 *  pane snapshots it at send time. detail: CodeSnapshot. */
export const CODE_CHANGED = "synapse:code-changed";
export interface CodeSnapshot {
  source: string;
  language: string;
}

/** Fired on window when the auth state flips (the auth store dispatches; gates re-render). */
export const AUTH_CHANGED = "synapse:auth-changed";

/** Fired on window to open the reader's nav drawer (the book's contents). The problem page's
 *  docked nav bar has no sidebar column of its own — its Contents pill dispatches this and
 *  `reader.ts` (already loaded for progress/prefs) opens the same drawer the mobile FAB drives.
 *  One drawer, two triggers; the event is the seam because the two live in different islands. */
export const OPEN_CONTENTS = "synapse:open-contents";

/** The relayout nudge — panes that unhide a Monaco fire it so the editor re-measures. */
export const RELAYOUT = "synapse:relayout";

/** Fired on window when the lazy viz loader has installed `__synapseViz` — workbenches
 *  re-render so the Visualise button appears (its presence is a render-time check). */
export const VIZ_READY = "synapse:viz-ready";

/** Fired on window by surfaces that render markdown LATE (the editorial pane) and may have
 *  planted fresh `.viz-widget`s — the viz loader re-sweeps (mounting is marker-idempotent),
 *  loading the wasm first if the page had no reason to before. */
export const VIZ_RESCAN = "synapse:viz-rescan";

declare global {
  interface Window {
    /** The auth store installs the real provider; absent = anonymous. */
    __synapseAuth?: () => boolean;
    /** The signed-in handle, or null when anonymous. Separate from `__synapseAuth` because the
     *  gates only ever need the boolean, while anything that KEYS per-account storage needs the
     *  name — the codebench's draft does. Same seam rather than an import: the codebench modal
     *  mounts on every page kind, and pulling in the auth store would drag its module graph into
     *  every page's eager bundle. */
    __synapseUser?: () => string | null;
    /** The viz loader installs the viz entry; its presence is what makes Visualise render at all. */
    __synapseViz?: (detail: {
      language: string;
      source: string;
      vizHint: string;
      stdin: string;
    }) => void;
    /** The viz crate's bearer, indirected: the auth store sets THIS; the viz loader hands the
     *  wasm a wrapper that reads it per-request — so identity and the lazy wasm can load in
     *  either order and a token refresh needs no re-install. */
    __synapseVizToken?: () => string | null;
    /** The docked player, installed by the same loader alongside `__synapseViz` — the `/viz`
     *  page's seam into the wasm. The page owns the buffer, the stdin and the structure; the
     *  panel owns the canvas and the playback, and the two only ever meet here. `mount` is
     *  idempotent, and every verb answers `false`/`null` rather than throwing when the panel is
     *  not up yet, because the page renders before the wasm arrives. */
    __synapseVizPanel?: {
      mount: (host: HTMLElement) => boolean;
      /** The page's SECOND wasm surface: the call stack, the program's output and the prompt a
       *  waiting program is stopped at. It goes under the editor rather than under the canvas,
       *  because every one of those reads the code. Same store as `mount`, so the two cannot
       *  disagree about which step is on screen. */
      mountConsole: (host: HTMLElement) => boolean;
      trace: (detail: {
        language: string;
        source: string;
        vizHint: string;
        stdin: string;
      }) => boolean;
      /** Follow the reader's step: the line that just EXECUTED and the line about to, both
       *  1-indexed, either null when there is no such line. The crate reports lines and nothing
       *  else — what a painted line looks like is the page's business, and the page is the one
       *  holding the editor. One listener; a second call replaces the first. */
      onCursor: (listener: (executed: number | null, next: number | null) => void) => void;
      /** The step on screen, or the whole walkthrough, as d2 SOURCE (no fence — the page owns
       *  the markdown wrapper). Null when there is nothing traced to export. */
      exportD2: (mode: "step" | "walkthrough") => string | null;
      /** The structure tokens the crate can draw — the picker's only source. */
      structures: () => string[];
      /** Take the trace off both surfaces. For when the traced code is EDITED: a trace of other
       *  code paints its arrows onto the wrong lines, and its prompt would answer for a program
       *  that no longer exists. */
      clear: () => void;
    };
  }
}

export function isAuthed(): boolean {
  return window.__synapseAuth?.() ?? false;
}

/** The signed-in handle, or null when anonymous (or before the auth store has installed its
 *  provider). Read per call — a sign-in mid-session changes the answer without a re-install. */
export function currentUser(): string | null {
  return window.__synapseUser?.() ?? null;
}
