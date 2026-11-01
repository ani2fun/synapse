// ──────────────────────────────────────────────────────────────────
// AUTHORED JSON BLOCKS
// The ```testcases and ```quiz fences a lesson carries as JSON, parsed and shape-checked
// ──────────────────────────────────────────────────────────────────
// Strict on purpose: render.ts turns every error into a visible card above the raw fence, so an
// authoring mistake is seen rather than silently degrading the block.

// ── Test cases ────────────────────────────────────────────────────
// The shapes mirror shared/src/execution/test_run.rs's ArgSpec/TestCase/TestSpec — the authored
// JSON uses `type` (mapped to the Rust DTO's `tpe` field at the codec).
type ArgSpecJson = { id: string; label: string; type: string; placeholder?: string };
type TestCaseJson = { args: Record<string, string>; expected?: string };
export type TestSpecJson = { args: ArgSpecJson[]; cases: TestCaseJson[] };

/**
 * Parse + shape-check an authored ```testcases fence. Strict on purpose:
 * an invalid fence renders a visible `.workbench-error` card (and the raw
 * fence below it) so authors see the mistake — never a silently-degraded
 * panel.
 */
export function parseTestSpec(raw: string): { spec?: TestSpecJson; error?: string } {
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch (e) {
    return { error: `not valid JSON (${(e as Error).message})` };
  }
  const isStr = (v: unknown): v is string => typeof v === "string";
  const isObj = (v: unknown): v is Record<string, unknown> =>
    typeof v === "object" && v !== null && !Array.isArray(v);
  if (!isObj(data)) return { error: "the top level must be an object with `args` and `cases`" };
  const { args, cases } = data;
  if (!Array.isArray(args)) return { error: "`args` must be an array of {id, label, type}" };
  for (const a of args) {
    const okPlaceholder = !isObj(a) || a.placeholder === undefined || isStr(a.placeholder);
    if (!isObj(a) || !isStr(a.id) || !isStr(a.label) || !isStr(a.type) || !okPlaceholder)
      return { error: "every arg needs string `id`, `label`, `type` (+ optional string `placeholder`)" };
  }
  if (!Array.isArray(cases) || cases.length === 0) return { error: "`cases` must be a non-empty array" };
  for (const c of cases) {
    const okExpected = !isObj(c) || c.expected === undefined || isStr(c.expected);
    if (!isObj(c) || !isObj(c.args) || Object.values(c.args).some((v) => !isStr(v)) || !okExpected)
      return { error: "every case needs an `args` object of strings (+ optional string `expected`)" };
  }
  return { spec: data as unknown as TestSpecJson };
}

// ── Quiz blocks ───────────────────────────────────────────────────
// A ```quiz fence carries one check-your-understanding card as JSON:
// {prompt, options: [String], answer: String (one of the options), input?}.
// Client-only — it never crosses the wire, so the model lives in the
// client, not in shared.

export type QuizJson = { prompt: string; options: string[]; answer: string; input?: string };

/** Parse + shape-check a ```quiz fence — strict, so authoring mistakes surface as a visible card. */
export function parseQuiz(raw: string): { quiz?: QuizJson; error?: string } {
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch (e) {
    return { error: `not valid JSON (${(e as Error).message})` };
  }
  const isStr = (v: unknown): v is string => typeof v === "string";
  const isObj = (v: unknown): v is Record<string, unknown> =>
    typeof v === "object" && v !== null && !Array.isArray(v);
  if (!isObj(data)) return { error: "the top level must be an object" };
  const { prompt, options, answer, input } = data;
  if (!isStr(prompt) || prompt.trim() === "") return { error: "`prompt` must be a non-empty string" };
  if (!Array.isArray(options) || options.length < 2 || options.some((o) => !isStr(o)))
    return { error: "`options` must be an array of at least two strings" };
  if (!isStr(answer)) return { error: "`answer` must be a string" };
  if (!options.includes(answer)) return { error: "`answer` must be one of the `options`" };
  if (input !== undefined && !isStr(input)) return { error: "`input`, when present, must be a string" };
  return { quiz: data as unknown as QuizJson };
}
