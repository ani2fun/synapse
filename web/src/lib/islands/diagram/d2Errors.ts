// ──────────────────────────────────────────────────────────────────
// WHAT D2 SAYS WHEN IT CANNOT COMPILE
// ──────────────────────────────────────────────────────────────────
// The engine rejects with a JSON array of ranges and messages, and the message itself carries the
// position again in its own prefix:
//
//   [{"range":"index,0:7:7-0:8:8","errmsg":"index:1:8: maps must be terminated with }"}]
//
// Shown raw — which is what `error.message` gives you — that is the whole array, braces and all,
// in a strip meant for one line of prose. Parsing it turns the same failure into something an
// author can act on: a sentence, and a line to jump to.
//
// Every step degrades rather than throws. A d2 upgrade that changes this shape must cost the jump
// affordance, not the error report.

/** One thing d2 objected to. */
export interface D2Problem {
  /** 1-based, matching the editor's gutter. Null when the message carried no position. */
  line: number | null;
  column: number | null;
  /** The message with its `index:1:8:` prefix removed. */
  message: string;
}

/** `index:1:8: maps must be terminated with }` → the position and the sentence after it. */
const PREFIX = /^[^\s:]*:(\d+):(\d+):\s*/;

/** A last-resort description of a value that is not going to parse. */
function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value); // circular, or a getter that throws
  }
}

function fromErrmsg(errmsg: string): D2Problem {
  const at = PREFIX.exec(errmsg);
  if (at == null) return { line: null, column: null, message: errmsg.trim() };
  return {
    line: Number(at[1]),
    column: Number(at[2]),
    message: errmsg.slice(at[0].length).trim(),
  };
}

/**
 * Everything d2 objected to, in source order, or a single unparsed problem carrying whatever the
 * engine said.
 *
 * The input is an unknown because it arrives from a rejected promise: usually an `Error` whose
 * message is the JSON, sometimes the array itself, and — if the engine ever changes — anything at
 * all. All three land somewhere useful.
 */
export function d2Problems(error: unknown): D2Problem[] {
  // Two shapes reach here: an Error whose message is the JSON (what the engine rejects with), and
  // the decoded array itself (what a caller that already parsed it would pass). Stringifying the
  // second would turn it into "[object Object]", so it is taken as-is.
  const decoded = typeof error === "object" && error !== null && !(error instanceof Error);
  const text = decoded
    ? safeStringify(error)
    : (error instanceof Error ? error.message : String(error)).trim();

  let parsed: unknown = decoded ? error : null;
  if (!decoded && (text.startsWith("[") || text.startsWith("{"))) {
    try {
      parsed = JSON.parse(text);
    } catch {
      parsed = null;
    }
  }

  const list = Array.isArray(parsed) ? parsed : parsed == null ? [] : [parsed];
  const problems = list
    .map((item) => (item as { errmsg?: unknown })?.errmsg)
    .filter((errmsg): errmsg is string => typeof errmsg === "string" && errmsg.trim() !== "")
    .map(fromErrmsg);

  // Nothing recognisable — report what there is rather than an empty strip that says a diagram
  // failed without saying why.
  return problems.length > 0 ? problems : [{ line: null, column: null, message: text }];
}

/** The one problem to lead with: the earliest in the file, since later ones are often its echo. */
export function firstProblem(error: unknown): D2Problem {
  const problems = d2Problems(error);
  return problems.reduce((best, next) =>
    (next.line ?? Infinity) < (best.line ?? Infinity) ? next : best,
  );
}
