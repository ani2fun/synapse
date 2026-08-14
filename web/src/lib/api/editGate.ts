// ──────────────────────────────────────────────────────────────────
// MAY THIS READER EDIT? — asked once per page
// ──────────────────────────────────────────────────────────────────
// `editConfig()` folds three things into one answer — editing is enabled here, you are signed in,
// you are on the content-editor list — and a lesson page now has several affordances that need
// it: the page's own "Suggest an edit", and an Edit pill on every d2 figure. Asking per
// affordance would put one request per diagram on the wire to learn the same fact.
//
// The answer can change after first paint (check-sso resolves late, someone signs in mid-page),
// so this is a memo with an invalidation rather than a one-shot snapshot: `AUTH_CHANGED` drops
// the cached promise and every subscriber is told again.

import * as api from "./client";
import type { EditConfig } from "./client";
import * as log from "../log";
import { AUTH_CHANGED } from "../../islands/workbench/contracts";

/** What an affordance needs to decide between live, gated, and absent. */
export interface EditGate {
  /** The deployment offers in-app editing at all. False → the affordance should go away. */
  enabled: boolean;
  /** This caller may use it. False → gated, with the instructions for asking. */
  canEdit: boolean;
}

/** Editing off is the safe default: a gated affordance is honest, a live one that 403s is not. */
const CLOSED: EditGate = { enabled: false, canEdit: false };

let pending: Promise<EditGate> | null = null;
const listeners = new Set<(gate: EditGate) => void>();

async function ask(): Promise<EditGate> {
  try {
    const config: EditConfig = await api.editConfig();
    return { enabled: config.enabled, canEdit: config.canEdit };
  } catch {
    // The routes are absent when `CONTENT_FORGE=off` — a 404 is the whole-feature answer, not a
    // per-resource one, and it is indistinguishable here from a request that simply failed.
    log.debug("edit gate: config unavailable — treating editing as off");
    return CLOSED;
  }
}

/** The gate, asked at most once per page until auth changes. */
export function editGate(): Promise<EditGate> {
  pending ??= ask();
  return pending;
}

/**
 * Subscribe to the gate, called immediately with the current answer and again whenever auth
 * flips. Returns an unsubscribe.
 */
export function onEditGate(listener: (gate: EditGate) => void): () => void {
  listeners.add(listener);
  void editGate().then((gate) => {
    if (listeners.has(listener)) listener(gate);
  });
  return () => listeners.delete(listener);
}

// One listener for the whole page, installed on first import rather than per subscriber.
if (typeof window !== "undefined") {
  window.addEventListener(AUTH_CHANGED, () => {
    pending = null;
    void editGate().then((gate) => {
      for (const listener of listeners) listener(gate);
    });
  });
}
