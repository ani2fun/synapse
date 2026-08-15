// ──────────────────────────────────────────────────────────────────
// WHAT MERMAID SAYS WHEN IT CANNOT PARSE
// ──────────────────────────────────────────────────────────────────
// mermaid's parsers are jison-generated, and a syntax error arrives as a plain `Error` carrying a
// `hash` — `{ text, token, line, loc, expected }` — with a message built from the same facts:
//
//   Parse error on line 2:
//   graph TD;  A --> ;
//   -----------------^
//   Expecting 'NODE_STRING', 'SPACE', 'BRKT', …thirty more…, got 'SEMI'
//
// Shown raw — which is what `error.message` gives you — that is four lines of caret art and a
// token census, in a strip meant for one line of prose. Parsing it turns the same failure into a
// sentence and a line to jump to.
//
// Two positions are on offer and they disagree: `hash.line` is the lexer's `yylineno`, which is
// ZERO-based, while the message prints `yylineno + 1`. The hash is preferred because it is data
// rather than prose, and +1 is applied here.
//
// Every step degrades rather than throws. A mermaid upgrade that changes this shape must cost the
// jump affordance, not the error report.

/** One thing mermaid objected to. */
export interface MermaidProblem {
  /** 1-based, matching the editor's gutter. Null when nothing carried a position. */
  line: number | null;
  /** A single sentence — no caret art, no echoed source. */
  message: string;
}

/** `Parse error on line 3:` — the prefix the message repeats the hash's position in. */
const PREFIX = /^Parse error on line (\d+):\s*/;

/** How many expected tokens are worth naming. jison lists every one it knows; a strip cannot hold
 *  forty, and the first few already say what kind of thing belongs here. */
const EXPECTED_SHOWN = 3;

/**
 * `No diagram type detected matching given configuration for text: <THE WHOLE SOURCE>`.
 *
 * That tail is the entire fence, newlines and all. It is the one message that must be cut rather
 * than trimmed — pasting a 40-line diagram into the error strip buries the sentence that matters.
 */
const ECHOES_SOURCE = /^(No diagram type detected)[\s\S]*$/;

/**
 * The author's line for a line the PARSER counted.
 *
 * mermaid never lexes the text it was handed. `preprocessDiagram` first pulls off a leading `---`
 * frontmatter block, then `%%{init: …}%%` directives, then every `%%` comment line — and only
 * then does the lexer start counting. So a diagram with one comment above the mistake reports it
 * one line high, and "Go to line N" lands on the wrong row: quieter than no jump at all, and
 * worse, because it looks right.
 *
 * Rather than replicate mermaid's rewrite, this walks the author's lines and counts the ones that
 * SURVIVE it — the only thing a line number needs. Unmappable input returns null and simply costs
 * the jump button, which is the honest failure.
 */
export function authorLineFor(source: string, parserLine: number): number | null {
  const lines = source.split("\n");
  let at = 0;

  // The frontmatter block, only when the very first line opens one.
  if (lines[0]?.trim() === "---") {
    const close = lines.findIndex((line, i) => i >= 1 && line.trim() === "---");
    if (close === -1) return null; // unterminated: mermaid's regex does not match, so do not guess
    at = close + 1;
  }
  // `trimStart()` runs last, so the blank lines above the first real content go too.
  while (at < lines.length && lines[at]!.trim() === "") at += 1;

  let survived = 0;
  for (let i = at; i < lines.length; i += 1) {
    // One predicate for both comments and directives: mermaid strips every `%%`-opening line.
    if (/^\s*%%/.test(lines[i]!)) continue;
    survived += 1;
    if (survived === parserLine) return i + 1; // 1-based, matching the gutter
  }
  return null;
}

/** The lexer's own hash, when the error carried one. */
function lineFromHash(error: unknown): number | null {
  const hash = (error as { hash?: { line?: unknown } } | null)?.hash;
  const line = hash?.line;
  // Zero-based, and only meaningful as a non-negative integer — a hash from a future mermaid that
  // spells this differently must fall through to the message rather than produce line 0.
  return typeof line === "number" && Number.isInteger(line) && line >= 0 ? line + 1 : null;
}

/** The raw text of whatever was thrown. `DetailedError` carries `str`; a jison error, `message`. */
function textOf(error: unknown): string {
  if (typeof error === "object" && error !== null) {
    const detailed = error as { str?: unknown; message?: unknown };
    if (typeof detailed.str === "string" && detailed.str.trim() !== "") return detailed.str.trim();
    if (typeof detailed.message === "string") return detailed.message.trim();
  }
  return String(error).trim();
}

/**
 * The actionable sentence inside a multi-line parse error.
 *
 * The caret art sits between the prefix and the verdict, so the LAST non-empty line is the one
 * worth keeping — `Expecting …, got 'X'` or `Unexpected end of input`. A message that was only
 * ever one line is returned as it is.
 */
function sentenceOf(body: string): string {
  const lines = body.split("\n").map((line) => line.trim()).filter((line) => line !== "");
  const last = lines[lines.length - 1] ?? body.trim();
  const expecting = /^Expecting (.+), got (.+)$/.exec(last);
  if (expecting == null) return last;
  const wanted = expecting[1]!.split(", ");
  const shown = wanted.slice(0, EXPECTED_SHOWN).join(", ");
  const tail = wanted.length > EXPECTED_SHOWN ? `${shown}…` : shown;
  return `Expecting ${tail} — got ${expecting[2]!}`;
}

/**
 * What mermaid objected to, as a sentence and a line.
 *
 * The input is an unknown because it arrives from a rejected promise: usually a jison `Error` with
 * a `hash`, sometimes mermaid's own `UnknownDiagramError`, and — if mermaid ever changes — anything
 * at all. All three land somewhere useful.
 *
 * `source` is the buffer the author is looking at. Pass it and the line is translated back out of
 * the parser's preprocessed copy (see `authorLineFor`); omit it and the line is reported as the
 * parser counted it, which is right only for a diagram with no comments or frontmatter.
 */
export function firstProblem(error: unknown, source?: string): MermaidProblem {
  const text = textOf(error);

  if (ECHOES_SOURCE.test(text)) {
    return {
      line: null,
      message: "No diagram type detected — the first line must name one, like `flowchart LR`.",
    };
  }

  const at = PREFIX.exec(text);
  const parserLine = lineFromHash(error) ?? (at == null ? null : Number(at[1]));
  const line =
    parserLine == null || source == null ? parserLine : authorLineFor(source, parserLine);
  const body = at == null ? text : text.slice(at[0].length);
  const message = sentenceOf(body);
  // Nothing recognisable — report what there is rather than an empty strip that says a diagram
  // failed without saying why.
  return { line, message: message === "" ? text : message };
}
