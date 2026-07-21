// Ten cases covering the `CodeExecutor` FSM's transitions and staleness guards.

import { describe, expect, it } from "vitest";
import type { components } from "../api/schema.gen";
import {
  cancel,
  cancelEdit,
  changedLineCount,
  clearOutcome,
  completed,
  enterEdit,
  failed,
  initial,
  isDirty,
  setCode,
  started,
} from "./executor";

type RunResult = components["schemas"]["RunResult"];

function result(stdout: string): RunResult {
  return {
    status: "Accepted",
    stdout,
    stderr: "",
    compileOutput: "",
    timeSeconds: null,
    memoryKb: null,
  };
}

describe("executor", () => {
  it("initialStateIsIdleReadonlyAndEmpty", () => {
    const state = initial("print(1)");
    expect(state.code).toBe("print(1)");
    expect(state.runState).toBe("idle");
    expect(state.editMode).toBe("readOnly");
    expect(state.result).toBeNull();
    expect(state.error).toBeNull();
  });

  it("startedClearsThePreviousOutcomeAndMintsANewHandle", () => {
    let done = started(initial("x"));
    const handle = done.runId;
    done = completed(done, handle, result("42"));
    const restarted = started(done);
    expect(restarted.runState).toBe("running");
    expect(restarted.result).toBeNull();
    expect(restarted.error).toBeNull();
    expect(restarted.runId).not.toBe(handle);
  });

  it("completedAppliesOnlyOnAMatchingHandle", () => {
    const running = started(initial("x"));
    const live = running.runId;
    const done = completed(running, live, result("42"));
    expect(done.runState).toBe("done");
    expect(done.result?.stdout).toBe("42");
  });

  it("staleResultsAndFailuresAreNoOps", () => {
    const first = started(initial("x"));
    const staleHandle = first.runId;
    const second = started(first); // restart: the first run's handle is now stale
    const afterStaleResult = completed(second, staleHandle, result("stale"));
    expect(afterStaleResult).toEqual(second);
    const afterStaleFailure = failed(second, staleHandle, "stale error");
    expect(afterStaleFailure).toEqual(second);
  });

  it("aResultForACancelledRunIsIgnored", () => {
    const running = started(initial("x"));
    const inFlight = running.runId;
    const cancelled = cancel(running);
    expect(cancelled.runState).toBe("idle");
    const late = completed(cancelled, inFlight, result("late"));
    expect(late).toEqual(cancelled);
  });

  it("failedRecordsTheErrorOnAMatchingHandle", () => {
    const running = started(initial("x"));
    const wasFailed = failed(running, running.runId, "backend down");
    expect(wasFailed.runState).toBe("done");
    expect(wasFailed.error).toBe("backend down");
  });

  it("setCodeTouchesNothingElse", () => {
    const running = started(initial("a"));
    const typed = setCode(running, "b");
    expect(typed.code).toBe("b");
    expect(typed.runState).toBe("running");
    expect(typed.runId).toBe(running.runId);
  });

  it("editModeTogglesAndCancelEditRevertsCodeButKeepsTheResult", () => {
    const state = initial("authored");
    const editing = enterEdit(state);
    expect(editing.editMode).toBe("editing");
    const ran = started(setCode(editing, "hacked"));
    const done = completed(ran, ran.runId, result("42"));
    const reverted = cancelEdit(done, "authored");
    expect(reverted.code).toBe("authored");
    expect(reverted.editMode).toBe("readOnly");
    expect(reverted.result?.stdout).toBe("42");
  });

  it("dirtinessAndChangedLinesCompareAgainstTheAuthoredSource", () => {
    const state = initial("a\nb\nc");
    expect(isDirty(state, "a\nb\nc")).toBe(false);
    const edited = setCode(state, "a\nX\nc\nd");
    expect(isDirty(edited, "a\nb\nc")).toBe(true);
    expect(changedLineCount(edited, "a\nb\nc")).toBe(2);
  });

  it("clearOutcomeDropsThePanelButKeepsCodeAndStaleGuardsInflightRuns", () => {
    const state = setCode(enterEdit(initial("authored")), "edited");
    const ran = started(state);
    const done = completed(ran, ran.runId, result("42"));
    const cleared = clearOutcome(done);
    expect(cleared.runState).toBe("idle");
    expect(cleared.result).toBeNull();
    expect(cleared.error).toBeNull();
    expect(cleared.code).toBe("edited");
    expect(cleared.editMode).toBe("editing");
    // A reply still in flight for the old handle must not resurrect the panel.
    const resurrected = completed(cleared, ran.runId, result("stale"));
    expect(resurrected.result).toBeNull();
  });
});
