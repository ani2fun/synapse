// The `CodeExecutor` FSM — pure `State → State` transitions for one runnable block. The
// staleness trick: there is no real HTTP cancel, so `cancel`/`started` bump the opaque
// `RunHandle` and a late result whose handle no longer matches is a NO-OP (`completed`/`failed`
// return the state unchanged).
//
// Functions take the state as their first argument rather than living as methods on a class —
// matching this codebase's other pure-logic modules (`catalog/progress.ts`, `catalog/prefs.ts`),
// where state is plain data and behaviour is free functions over it.

import type { components } from "../api/schema.gen";

type RunResult = components["schemas"]["RunResult"];

export type RunState = "idle" | "running" | "done";

/** Orthogonal to `RunState`; the auth gate is enforced by the CALLER (the identity island), not
 *  by the FSM. */
export type EditMode = "readOnly" | "editing";

// Opaque-ish, monotonic — a branded number rather than a bare one, so a raw number cannot be
// assigned where a handle is expected by accident. True opacity would need a WeakMap/closure
// trick (TS has no private-field module boundary for a numeric type); that would be overkill for
// an ID that only ever needs `===` comparison, so this is a deliberate, lighter guarantee.
declare const runHandleBrand: unique symbol;
export type RunHandle = number & { readonly [runHandleBrand]: never };

const INITIAL_HANDLE = 0 as RunHandle;

function nextHandle(handle: RunHandle): RunHandle {
  return ((handle as number) + 1) as RunHandle;
}

export interface ExecutorState {
  code: string;
  runState: RunState;
  editMode: EditMode;
  result: RunResult | null;
  error: string | null;
  runId: RunHandle;
}

export function initial(source: string): ExecutorState {
  return {
    code: source,
    runState: "idle",
    editMode: "readOnly",
    result: null,
    error: null,
    runId: INITIAL_HANDLE,
  };
}

/** Reset-to-starter: identical to `initial` (kept as its own verb for callers). */
export function reset(source: string): ExecutorState {
  return initial(source);
}

/** A run begins: clear the previous outcome, mint the handle the eventual result must show. */
export function started(state: ExecutorState): ExecutorState {
  return {
    ...state,
    runState: "running",
    result: null,
    error: null,
    runId: nextHandle(state.runId),
  };
}

/** Clear the run outcome (case switch): the buffer and edit unlock survive, the stale
 *  result/error panel disappears, and the bumped handle stale-guards any run in flight — its
 *  reply must not resurrect the panel under the newly selected case. */
export function clearOutcome(state: ExecutorState): ExecutorState {
  return {
    ...state,
    runState: "idle",
    result: null,
    error: null,
    runId: nextHandle(state.runId),
  };
}

/** Cancel without a real HTTP cancel: back to idle and BUMP the handle, so the in-flight run's
 *  eventual result is stale on arrival. */
export function cancel(state: ExecutorState): ExecutorState {
  return {
    ...state,
    runState: "idle",
    runId: nextHandle(state.runId),
  };
}

/** Apply a result — only if it belongs to the CURRENT run. */
export function completed(state: ExecutorState, handle: RunHandle, result: RunResult): ExecutorState {
  if (state.runId !== handle) return state;
  return {
    ...state,
    runState: "done",
    result,
  };
}

/** Apply a failure — same staleness guard. */
export function failed(state: ExecutorState, handle: RunHandle, error: string): ExecutorState {
  if (state.runId !== handle) return state;
  return {
    ...state,
    runState: "done",
    error,
  };
}

/** Buffer edits touch NOTHING else — a keystroke during a run must not eat the result. */
export function setCode(state: ExecutorState, code: string): ExecutorState {
  return { ...state, code };
}

export function enterEdit(state: ExecutorState): ExecutorState {
  return { ...state, editMode: "editing" };
}

/** Leave edit mode reverting the buffer to the authored source — the last RESULT survives
 *  (reverting code is not un-running it). */
export function cancelEdit(state: ExecutorState, source: string): ExecutorState {
  return { ...state, code: source, editMode: "readOnly" };
}

export function isDirty(state: ExecutorState, source: string): boolean {
  return state.code !== source;
}

/** How many lines differ from the authored source, by index (the Edit chrome's badge). */
export function changedLineCount(state: ExecutorState, source: string): number {
  const current = state.code.split("\n");
  const authored = source.split("\n");
  const max = Math.max(current.length, authored.length);
  let changed = 0;
  for (let i = 0; i < max; i += 1) {
    if (current[i] !== authored[i]) changed += 1;
  }
  return changed;
}
