// The authored test suite + pure judging — mirrors `judge` + `stdin_for` in
// shared/src/execution/test_run.rs, used by BOTH the server and the workbench island.
// `ArgSpec`/`TestCase`/`TestSpec` never cross the wire through a utoipa-documented endpoint
// (unlike `RunResult`/`RunStatus`, which live in `../api/schema.gen`), so they are defined here
// rather than generated — this module is their TS home, same as `test_run.rs` is their Rust one.
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

/** One authored case: values per declared arg + the optional expected stdout. */
export interface TestCase {
  args: Record<string, string>;
  expected?: string | null;
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

/** Judge one run: a non-clean run is `Errored`; a clean run with no expected output is
 *  `Finished`; otherwise TRIMMED stdout comparison. */
export function judge(result: RunResult, expected: string | null | undefined): Verdict {
  if (result.status !== "Accepted") return "Errored";
  if (expected == null) return "Finished";
  return result.stdout.trim() === expected.trim() ? "Accepted" : "WrongAnswer";
}
