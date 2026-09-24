// The authored test suite + pure judging — mirrors `judge` + `stdin_for` in
// shared/src/execution/test_run.rs, used by BOTH the server and the workbench island.
// This hand-written `ArgSpec`/`TestCase`/`TestSpec` is the workbench's JUDGING vocabulary and its
// TS home, same as `test_run.rs` is their Rust one. A SAMPLE projection of `TestSpec` also crosses
// the wire on `LessonPayloadDto.tests` (the generated `components["schemas"]["TestSpec"]` in
// `../api/schema.gen`); `islands/problem` parses that payload back into this structurally identical
// shape, so the two stay in lock-step by design.
//
// Kept in lock-step with the Rust twin by `shared/test-vectors/judge-vectors.json`: both
// `judge.test.ts` (here) and `shared/src/execution/judge_vectors_test.rs` run the SAME vectors,
// so a change to either side's rules fails a test on both.

import type { components } from "../api/schema.gen";

type RunResult = components["schemas"]["RunResult"];

/** One declared stdin argument. The Rust DTO's field is `tpe` (mapped to the wire's `type` at
 *  the codec, avoiding a keyword clash); TS has no such keyword clash, so the field here is just
 *  `type`. */
export interface ArgSpec {
  id: string;
  label: string;
  type: string;
  placeholder?: string | null;
}

/** One authored case: values per declared arg + the optional expected stdout. `sample` marks a
 *  browser-visible case; hidden judge cases never reach the client, so every case in a parsed
 *  payload is a sample and the flag is informational here (the workbench judges every case it holds). */
export interface TestCase {
  args: Record<string, string>;
  expected?: string | null;
  sample?: boolean | null;
}

/** The whole authored suite (a testcases fence or a `.tests.json` sidecar). */
export interface TestSpec {
  args: ArgSpec[];
  cases: TestCase[];
}

/** A judged case's verdict — spelled as the case-name string, matching how `RunStatus` already
 *  crosses the wire. This type never itself crosses the wire; the string form is a deliberate
 *  choice for consistency with the rest of this module's vocabulary, not a wire requirement. */
export type Verdict = "Accepted" | "WrongAnswer" | "Errored" | "Finished";

/** The stdin a case feeds the program: ONE LINE PER DECLARED ARG, in declaration order (missing
 *  values become empty lines), with a trailing newline. */
export function stdinFor(args: ArgSpec[], values: Record<string, string>): string {
  const lines = args.map((arg) => values[arg.id] ?? "");
  lines.push(""); // the trailing newline
  return lines.join("\n");
}

/**
 * Did the run die because stdin ran out?
 *
 * The commonest way a program fails on a page where the input is a box someone types into, and
 * the one whose own message explains least: the runtimes name the mechanism (`EOFError`, a bare
 * `NoSuchElementException`) and never the cause, which is that the INPUT is short — not the code.
 * A reader who reads the traceback goes looking at their program.
 *
 * Java's exception is not exclusive to reading, so it counts only with `Scanner` in the trace;
 * an empty-collection `NoSuchElementException` is a different bug and must not be renamed.
 */
export function ranOutOfInput(result: RunResult): boolean {
  const stderr = result.stderr;
  if (stderr.includes("EOFError")) return true;
  return stderr.includes("NoSuchElementException") && stderr.includes("java.util.Scanner");
}

/**
 * What a program that ran out of input was ASKING when it stopped: the unfinished last line of its
 * stdout, which is where `input("How many? ")` and `System.out.print("How many? ")` leave a
 * question that has no newline yet. Null when the output ends at a line break — nothing is
 * pending on the line, so there is no question to quote.
 */
export function pendingPrompt(stdout: string): string | null {
  const tail = stdout.slice(stdout.lastIndexOf("\n") + 1).trim();
  return tail === "" ? null : tail;
}

/** Judge one run: a non-clean run is `Errored`; a clean run with no expected output is
 *  `Finished`; otherwise TRIMMED stdout comparison. */
export function judge(result: RunResult, expected: string | null | undefined): Verdict {
  if (result.status !== "Accepted") return "Errored";
  if (expected == null) return "Finished";
  return result.stdout.trim() === expected.trim() ? "Accepted" : "WrongAnswer";
}
