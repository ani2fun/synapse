/**
 * The workbench's reactive stores, over lib/store.ts. The FSM itself is pure (lib/execution/
 * executor.ts, test-pinned); these wrap it with I/O and the staleness discipline.
 */
import { run as apiRun, submit as apiSubmit, submission as apiSubmission } from "../../lib/api/client";
import type { Submission } from "../../lib/api/client";
import * as executor from "../../lib/execution/executor";
import type { ExecutorState } from "../../lib/execution/executor";
import type { TestCase, TestSpec } from "../../lib/execution/judge";
import { seedValues } from "../../lib/execution/blocks";
import type { Verdict } from "../../lib/execution/judge";
import * as log from "../../lib/log";
import { Store } from "../../lib/store";

/** One runnable block's state: the FSM in a store. Whether the buffer may be typed into is the
 *  workbench's auth check applied to the editor, not state held here. */
export class BlockStore {
  readonly state: Store<ExecutorState>;

  constructor(source: string) {
    this.state = new Store(executor.initial(source));
  }

  /**
   * Run the current buffer. Guards like the Run button: a run in flight wins. The reply is
   * applied through the FSM's handle check, so a stale response after a re-launch is a no-op —
   * the semantics the executor tests pin.
   */
  launch(language: string, stdin: string | null): void {
    const current = this.state.get();
    if (current.runState === "running") return;
    const startedState = executor.started(current);
    const handle = startedState.runId;
    const source = startedState.code;
    log.info(`running ${language} block${stdin != null ? " (with case stdin)" : ""}`);
    this.state.set(startedState);
    void (async () => {
      try {
        const result = await apiRun({ language, source, stdin });
        log.debug(`run done: ${result.status}`);
        this.state.update((s) => executor.completed(s, handle, result));
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        log.error(`run failed: ${message}`);
        this.state.update((s) => executor.failed(s, handle, message));
      }
    })();
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// SUBMIT — POST → poll every 1.2 s (≤ 100 tries), gated by `alive`
// ─────────────────────────────────────────────────────────────────────────────

export type SubmitState =
  | { kind: "idle" }
  | { kind: "judging"; id: string }
  | { kind: "done"; dto: Submission }
  | { kind: "failed"; message: string };

const POLL_MS = 1_200;
const POLL_TRIES = 100;

export class SubmitStore {
  readonly state = new Store<SubmitState>({ kind: "idle" });
  private alive = true;

  /** Called from the component's unmount cleanup — an unmounted block stops polling. */
  dispose(): void {
    this.alive = false;
  }

  /** Guarded like the button: one judging at a time. */
  submit(path: string[], language: string, source: string): void {
    if (this.state.get().kind === "judging") return;
    log.info(`submitting ${language} solution for ${path.join("/")}`);
    void (async () => {
      let id: string;
      try {
        id = (await apiSubmit({ path, language, source })).id;
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        log.error(`submit failed: ${message}`);
        this.state.set({ kind: "failed", message });
        return;
      }
      log.debug(`submission ${id} accepted — polling`);
      this.state.set({ kind: "judging", id });
      for (let attempt = 0; attempt < POLL_TRIES; attempt++) {
        await new Promise((resolve) => setTimeout(resolve, POLL_MS));
        if (!this.alive) {
          log.debug(`submission ${id}: poll cancelled (block unmounted)`);
          return;
        }
        try {
          const dto = await apiSubmission(id);
          if (dto.status === "completed") {
            log.info(
              `submission ${id}: ${dto.verdict ?? "?"} — ${dto.passed ?? 0}/${dto.total ?? 0}`,
            );
            this.state.set({ kind: "done", dto });
            return;
          }
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          this.state.set({ kind: "failed", message });
          return;
        }
      }
      log.error("judging timed out");
      this.state.set({ kind: "failed", message: "judging timed out" });
    })();
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTS — the panel's state + the one case sink
// ─────────────────────────────────────────────────────────────────────────────

/** Per-block test-panel state. `ranCase` is the case a launch was FIRED for, so the arriving
 *  result is judged against IT — never against whichever chip is selected by then. */
export class TestsState {
  readonly activeCase = new Store(0);
  readonly values: Store<Record<string, string>>;
  readonly verdicts = new Store<ReadonlyMap<number, Verdict>>(new Map());
  readonly ranCase = new Store<number | null>(null);
  /** The LIVE suite: authored cases plus any the learner appends. */
  readonly spec: Store<TestSpec>;

  /** `activeCase` is a parameter so a RESTORED suite can open on the chip the reader left on;
   *  it is the caller's job to have clamped it into range (`testsDraft.parse` does). */
  constructor(spec: TestSpec, activeCase = 0) {
    this.spec = new Store(spec);
    this.activeCase.set(activeCase);
    this.values = new Store(seedValues(spec, activeCase));
  }

  recordVerdict(caseIndex: number, verdict: Verdict): void {
    this.verdicts.update((map) => new Map(map).set(caseIndex, verdict));
  }

  /**
   * Write one input THROUGH to the live suite, not just to the scratch buffer.
   *
   * `values` is scratch: `switchTo` re-seeds it from `spec.cases[i].args`, so a value that only
   * ever reached `values` is discarded the moment the reader visits another chip and comes back —
   * no reload required. The suite is the durable thing, and this is the only writer that keeps the
   * two agreeing.
   */
  setValue(argId: string, value: string): void {
    this.values.update((v) => ({ ...v, [argId]: value }));
    const caseIndex = this.activeCase.get();
    this.spec.update((s) => ({
      ...s,
      cases: s.cases.map((testCase, i) =>
        i === caseIndex ? { ...testCase, args: { ...testCase.args, [argId]: value } } : testCase,
      ),
    }));
  }

  switchTo(caseIndex: number): void {
    this.activeCase.set(caseIndex);
    this.values.set(seedValues(this.spec.get(), caseIndex));
  }

  /**
   * APPEND, never insert: `verdicts` is a sparse map keyed by case index, so inserting
   * mid-list would slide every existing ✓/✗ onto the wrong chip. Returns the new index; the
   * caller runs the same on-switch path the chips use — the output panel judges against
   * `ranCase`, and without that a previous case's result would stay on screen, still labelled
   * with ITS expected, while the new chip sits selected.
   */
  append(testCase: TestCase): number {
    const index = this.spec.get().cases.length;
    this.spec.update((s) => ({ ...s, cases: [...s.cases, testCase] }));
    this.switchTo(index);
    return index;
  }
}
