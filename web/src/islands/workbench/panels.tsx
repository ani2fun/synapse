/**
 * The output, tests and verdict panels. Classes match runnable.css/practice.css exactly.
 */
import type { ComponentChildren } from "preact";

import type { Submission } from "../../lib/api/client";
import type { ExecutorState } from "../../lib/execution/executor";
import { expectedFor, canReproduce } from "../../lib/execution/blocks";
import { judge } from "../../lib/execution/judge";
import type { TestCase } from "../../lib/execution/judge";
import type { RunResult } from "../../lib/api/client";
import { useStore } from "../../lib/store";
import type { SubmitStore, TestsState } from "./state";

// ── Output ────────────────────────────────────────────────────────────────────

function StreamBlock({ label, content }: { label: string; content: string }) {
  if (content === "") return null;
  return (
    <details class="runnable__details" open>
      <summary class="runnable__details-label">{label}</summary>
      <pre class="runnable__stream">{content}</pre>
    </details>
  );
}

function ResultPanel({ result, expected }: { result: RunResult; expected: string | null }) {
  const verdict = expected != null ? judge(result, expected) : null;
  const badgeOk =
    verdict === "Accepted" || (verdict === null && result.status === "Accepted");
  const badgeLabel =
    verdict === "Accepted"
      ? "Accepted ✓"
      : verdict === "WrongAnswer"
        ? "Wrong answer ✗"
        : result.status;
  const stdoutClass =
    verdict === "Accepted"
      ? "runnable__stdout wb-legend--ok"
      : verdict === "WrongAnswer"
        ? "runnable__stdout wb-legend--err"
        : "runnable__stdout";
  const time = result.timeSeconds != null ? `${result.timeSeconds.toFixed(3)} s` : null;
  const memory = result.memoryKb != null ? `${Math.floor(result.memoryKb / 1024)} MB` : null;
  return (
    <div class="runnable__out">
      <div class="runnable__status">
        <span class={badgeOk ? "runnable__badge runnable__badge--ok" : "runnable__badge runnable__badge--fail"}>
          {badgeLabel}
        </span>
        {time && <span class="runnable__meta">{time}</span>}
        {memory && <span class="runnable__meta">{memory}</span>}
      </div>
      <StreamBlock label="compile output" content={result.compileOutput ?? ""} />
      <StreamBlock label="stderr" content={result.stderr} />
      {result.stdout === "" ? (
        <p class="runnable__empty">(no output)</p>
      ) : (
        <pre class={stdoutClass}>{result.stdout}</pre>
      )}
    </div>
  );
}

export function Output({ state, tests }: { state: ExecutorState; tests: TestsState | null }) {
  const ranCase = tests ? useStore(tests.ranCase) : null;
  const spec = tests ? useStore(tests.spec) : null;
  if (state.error != null) {
    return (
      <div class="runnable__out runnable__out--error">
        <div class="runnable__status">
          <span class="runnable__badge runnable__badge--fail">Error</span>
        </div>
        <pre class="runnable__stream">{state.error}</pre>
      </div>
    );
  }
  if (state.result != null) {
    // Judged against the case the run was LAUNCHED for — switching chips must never re-label
    // an old run's output under a different case's expected.
    const expected = spec != null && ranCase != null ? expectedFor(spec, ranCase) : null;
    return <ResultPanel result={state.result} expected={expected} />;
  }
  if (state.runState === "running") {
    return <div class="runnable__out runnable__out--running">Running…</div>;
  }
  return null;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

export function TestsPanel({
  tests,
  onSwitch,
}: {
  tests: TestsState;
  /** Fired on chip click / append AFTER the re-seed — the block clears stale run output. */
  onSwitch: (caseIndex: number) => void;
}) {
  const spec = useStore(tests.spec);
  const active = useStore(tests.activeCase);
  const values = useStore(tests.values);
  const verdicts = useStore(tests.verdicts);

  const chip = (index: number) => {
    const verdict = verdicts.get(index);
    const ok = verdict === "Accepted";
    const fail = verdict === "WrongAnswer" || verdict === "Errored";
    return (
      <button
        class={`wb__chip${active === index ? " wb__chip--active" : ""}${ok ? " wb__chip--ok" : ""}${fail ? " wb__chip--fail" : ""}`}
        onClick={() => {
          tests.switchTo(index);
          onSwitch(index);
        }}
      >
        {`Case ${index + 1}`}
        {ok && <span class="wb__tick">✓</span>}
        {fail && <span class="wb__tick">✗</span>}
      </button>
    );
  };

  const expected = expectedFor(spec, active);
  return (
    <div class="wb__tests">
      <div class="wb__chips">
        {spec.cases.map((_, index) => chip(index))}
        <button
          class="wb__chip wb__chip--add"
          aria-label="Add a test case"
          title="Add a test case of your own"
          onClick={() => {
            // An empty case has nothing to check — the chip stays unbadged until the code
            // actually crashes on it, which is the honest signal.
            const index = tests.append({ args: {}, expected: null });
            onSwitch(index);
          }}
        >
          +
        </button>
      </div>
      <div class="wb__values">
        {spec.args.map((arg) => (
          <label class="wb__field">
            <span class="wb__field-label">{arg.label}</span>
            <input
              class="wb__input"
              placeholder={arg.placeholder ?? ""}
              value={values[arg.id] ?? ""}
              onInput={(event) => {
                const value = (event.target as HTMLInputElement).value;
                tests.values.update((v) => ({ ...v, [arg.id]: value }));
              }}
            />
          </label>
        ))}
      </div>
      {expected != null && (
        <div class="wb__expected">
          <span class="wb__field-label">Expected</span>
          <pre>{expected}</pre>
        </div>
      )}
    </div>
  );
}

// ── Verdict ───────────────────────────────────────────────────────────────────

/** The button that turns a judged failure into a case you can Run — DISABLED rather than
 *  hidden when irreproducible, because a missing button reads as a bug and the tooltip is the
 *  only place the learner can find out why. */
function UseCaseButton({
  failure,
  tests,
  onSwitch,
}: {
  failure: NonNullable<Submission["firstFailure"]>;
  tests: TestsState | null;
  onSwitch: (caseIndex: number) => void;
}) {
  const reproducible = tests != null && canReproduce(tests.spec.get(), failure.args);
  const tip = reproducible
    ? "Adds this input as a new case below. It reproduces the input, not necessarily the judge's exact stdin."
    : "This problem is judged against a larger hidden suite whose inputs don't line up with the fields below.";
  return (
    <button
      class="wb__use-case"
      disabled={!reproducible}
      data-tip={tip}
      onClick={() => {
        if (!tests) return;
        const testCase: TestCase = { args: failure.args, expected: failure.expected ?? null };
        const index = tests.append(testCase);
        onSwitch(index);
      }}
    >
      Use this test case
    </button>
  );
}

export function VerdictPanel({
  submit,
  tests,
  onSwitch,
}: {
  submit: SubmitStore;
  tests: TestsState | null;
  onSwitch: (caseIndex: number) => void;
}) {
  const state = useStore(submit.state);
  if (state.kind === "idle") return null;
  if (state.kind === "judging") {
    return (
      <div class="wb__verdict wb__verdict--judging">
        Judging against the hidden suite… <span class="wb__verdict-id">{state.id}</span>
      </div>
    );
  }
  if (state.kind === "failed") {
    return <div class="wb__verdict wb__verdict--failed">Submit failed: {state.message}</div>;
  }
  const dto = state.dto;
  const counts = `${dto.passed ?? 0} / ${dto.total ?? 0}`;
  if (dto.verdict === "accepted") {
    return <div class="wb__verdict wb__verdict--accepted">Accepted ✓ — {counts} cases</div>;
  }
  if (dto.verdict === "rejected") {
    const failure = dto.firstFailure;
    let detail: ComponentChildren = null;
    if (failure) {
      detail = (
        <div class="wb__failure">
          <div class="wb__failure-head">
            <span class="wb__field-label">{`First failure — case ${failure.index + 1}`}</span>
            <UseCaseButton failure={failure} tests={tests} onSwitch={onSwitch} />
          </div>
          {Object.entries(failure.args).map(([id, value]) => (
            <pre class="wb__failure-line">{`${id}: ${value}`}</pre>
          ))}
          {failure.expected != null && <pre class="wb__failure-line">expected: {failure.expected}</pre>}
          <pre class="wb__failure-line">stdout:   {failure.stdout}</pre>
          {failure.stderr !== "" && <pre class="wb__failure-line">stderr:   {failure.stderr}</pre>}
        </div>
      );
    }
    return (
      <div class="wb__verdict wb__verdict--rejected">
        Wrong answer ✗ — {counts} cases passed
        {detail}
      </div>
    );
  }
  return (
    <div class="wb__verdict wb__verdict--failed">
      The judge failed mid-suite — {counts} passed. {dto.detail ?? ""}
    </div>
  );
}
