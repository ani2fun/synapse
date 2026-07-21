/**
 * The workbench's cross-island contracts (A06). The old client threaded RwSignals through
 * props (reader → hydrate → RunnableBlock); islands cannot share signals, so every seam
 * becomes a named CustomEvent or a window-scoped provider — ALL of them declared here, once,
 * because an event name in two files is a typo waiting to disagree.
 */

/** Dispatched ON a workbench root by A08's editorial (copy-to-editor). detail: LoadCode.
 *  The event itself is the tick — re-dispatching the same code fires again by construction,
 *  which is what the Rust needed a (tick, lang, code) triple to express. */
export const LOAD_CODE = "synapse:load-code";
export interface LoadCode {
  language: string;
  code: string;
}

/** Dispatched ON a workbench root by A07's Submissions rows (reproduce a failing input).
 *  detail: UseCase — the TestsPanel appends and selects it (step-63 semantics). */
export const USE_CASE = "synapse:use-case";
export interface UseCase {
  args: Record<string, string>;
  expected: string | null;
}

/** Dispatched (bubbling) FROM a workbench root when a submit lifecycle completes — A07's
 *  Submissions tab refetches on it. */
export const SUBMITTED = "synapse:submitted";

/** Dispatched (bubbling) FROM a workbench root on every buffer edit / tab switch — A09's
 *  coach snapshots it at send time. detail: CodeSnapshot. */
export const CODE_CHANGED = "synapse:code-changed";
export interface CodeSnapshot {
  source: string;
  language: string;
}

/** Fired on window when the auth state flips (A11 dispatches; gates re-render). */
export const AUTH_CHANGED = "synapse:auth-changed";

/** Fired on window to open the reader's nav drawer (the book's contents). A07's docked problem
 *  nav bar has no sidebar column of its own — its Contents pill dispatches this and `reader.ts`
 *  (already loaded for progress/prefs) opens the same drawer the mobile FAB drives. One drawer,
 *  two triggers; the event is the seam because the two live in different islands. */
export const OPEN_CONTENTS = "synapse:open-contents";

/** The relayout nudge (same name as the old client's RELAYOUT_EVENT) — panes that unhide a
 *  Monaco fire it so the editor re-measures. */
export const RELAYOUT = "synapse:relayout";

/** Fired on window when the lazy viz loader (A10) has installed `__synapseViz` — workbenches
 *  re-render so the Visualise button appears (its presence is a render-time check). */
export const VIZ_READY = "synapse:viz-ready";

/** Fired on window by surfaces that render markdown LATE (the editorial pane) and may have
 *  planted fresh `.viz-widget`s — the viz loader re-sweeps (mounting is marker-idempotent),
 *  loading the wasm first if the page had no reason to before. */
export const VIZ_RESCAN = "synapse:viz-rescan";

declare global {
  interface Window {
    /** A11 installs the real provider; absent = anonymous. Mirrors the Rust AuthStore seam. */
    __synapseAuth?: () => boolean;
    /** A10 installs the viz entry; its presence is what makes Visualise render at all. */
    __synapseViz?: (detail: {
      language: string;
      source: string;
      vizHint: string;
      stdin: string;
    }) => void;
    /** The viz crate's bearer, indirected: A11 sets THIS; the viz loader hands the wasm a
     *  wrapper that reads it per-request — so identity and the lazy wasm can load in either
     *  order and a token refresh needs no re-install. */
    __synapseVizToken?: () => string | null;
  }
}

export function isAuthed(): boolean {
  return window.__synapseAuth?.() ?? false;
}
