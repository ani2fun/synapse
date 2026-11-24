/**
 * The Output pane's body: what the last Run produced, or what is happening instead.
 *
 * A run that went badly is still a RESULT — the sandbox answered, it just said "compile error" —
 * so it renders with its streams, under a red badge. Only a run that never reached the sandbox
 * (signed out, throttled, network) is a `failed` state, and it shows the server's own sentence.
 *
 * A program that stopped because stdin ran dry is told apart from one that crashed: the traceback
 * names `EOFError`, which sends a reader hunting through their code, when the fix is a line in the
 * Input box. So it reads as waiting, quotes the question, and points there.
 */
import type { RunResult } from "../../lib/api/client";
import { pendingPrompt, ranOutOfInput } from "../../lib/execution/judge";
import { TERMINAL } from "./icons";

export type RunState =
  | { readonly kind: "idle" }
  | { readonly kind: "running"; readonly language: string }
  | { readonly kind: "done"; readonly result: RunResult; readonly language: string }
  | { readonly kind: "failed"; readonly message: string };

const STATUS_LABEL: Record<RunResult["status"], string> = {
  Accepted: "Finished",
  CompileError: "Compile error",
  RuntimeError: "Runtime error",
  TimeLimitExceeded: "Time limit exceeded",
  InternalError: "Sandbox error",
};

/** What the Output pane's download and copy buttons take: the streams as a reader would read them. */
export function outputText(state: RunState): string {
  if (state.kind === "failed") return state.message;
  if (state.kind !== "done") return "";
  const { compileOutput, stdout, stderr } = state.result;
  return [compileOutput, stdout, stderr].filter((part) => part.trim() !== "").join("\n");
}

function meta(result: RunResult): string {
  const parts: string[] = [];
  if (result.timeSeconds != null) parts.push(`${result.timeSeconds.toFixed(2)} s`);
  if (result.memoryKb != null) parts.push(`${(result.memoryKb / 1024).toFixed(1)} MB`);
  return parts.join(" · ");
}

function Stream({ label, text, tone }: { label: string; text: string; tone?: "err" }) {
  if (text.trim() === "") return null;
  return (
    <section class={`clab-out__stream${tone === "err" ? " clab-out__stream--err" : ""}`}>
      <h3 class="clab-out__label">{label}</h3>
      <pre class="clab-out__pre">{text}</pre>
    </section>
  );
}

export function Output({ state }: { state: RunState }) {
  if (state.kind === "idle") {
    return (
      <div class="clab-out__empty" data-codelab-empty>
        <span class="clab-out__glyph">{TERMINAL}</span>
        <p class="clab-out__empty-title">Ready to Run</p>
        <p class="clab-out__empty-note">Write your code and click Run to see the output here.</p>
      </div>
    );
  }
  if (state.kind === "running") {
    return (
      <div class="clab-out__empty clab-out__empty--busy" aria-live="polite">
        <p class="clab-out__empty-title">Running…</p>
        <p class="clab-out__empty-note">Compiling and running in the sandbox.</p>
      </div>
    );
  }
  if (state.kind === "failed") {
    return (
      <div class="clab-out__body" aria-live="polite">
        <div class="runnable__status">
          <span class="runnable__badge runnable__badge--fail">Not run</span>
        </div>
        <p class="clab-out__why">{state.message}</p>
      </div>
    );
  }

  const { result } = state;
  const waiting = result.status !== "Accepted" && ranOutOfInput(result);
  const badge = waiting ? "wait" : result.status === "Accepted" ? "ok" : "fail";
  const prompt = waiting ? pendingPrompt(result.stdout) : null;
  const silent = result.stdout === "" && result.stderr.trim() === "" && result.compileOutput.trim() === "";

  return (
    <div class="clab-out__body" aria-live="polite" data-codelab-status={result.status}>
      <div class="runnable__status">
        <span class={`runnable__badge runnable__badge--${badge}`}>
          {waiting ? "Waiting for input" : STATUS_LABEL[result.status]}
        </span>
        <span class="runnable__meta">{meta(result)}</span>
      </div>
      {waiting && (
        <p class="runnable__hint">
          {prompt != null ? <>The program asked “{prompt}” and </> : <>The program </>}
          ran out of input. Add a line to Input for each value it reads, then Run again.
        </p>
      )}
      <Stream label="Compiler" text={result.compileOutput} tone="err" />
      <Stream label="Output" text={result.stdout} />
      {waiting ? (
        result.stderr.trim() !== "" && (
          <details class="runnable__details">
            <summary class="runnable__details-label">Traceback</summary>
            <pre class="clab-out__pre">{result.stderr}</pre>
          </details>
        )
      ) : (
        <Stream label="Errors" text={result.stderr} tone="err" />
      )}
      {silent && <p class="runnable__empty">The program printed nothing.</p>}
    </div>
  );
}
