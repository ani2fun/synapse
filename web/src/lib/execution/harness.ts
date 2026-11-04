// The hidden I/O harness of a problem fence.
//
// A problem's runnable fence is a COMPLETE program, because the sandbox runs whole programs and
// feeds them stdin: imports, the `class Solution` the reader works on, and a driver that parses
// one stdin line per argument, calls the method and prints the result. Only the class is the
// reader's business. The driver is platform plumbing, and showing it makes a ten-line exercise
// look like a hundred-line one.
//
// So a problem's variant is SPLIT once, at decode: the editor shows the `class Solution` block
// (dedented, and without Java's `static` when the class is nested inside `Main`), and the rest is
// kept aside as the variant's `Harness`. Every path that sends code to a runner (Run, Submit,
// Visualise) JOINS the buffer back into the harness first, so the sandbox, the judge and the
// tracer still receive the same complete program the content repo authored and tested.
//
// The split is found, not declared: the content repo's fences are unchanged, and a fence with no
// single `class Solution` (a lesson listing, a design problem whose class has its own name) is
// left whole. Pure, so it is native-testable.

/** What a split keeps aside: everything around the class, and how to put the class back. */
export interface Harness {
  /** Lines before the class, or null when the class opens the fence. */
  prefix: string | null;
  /** Lines after the class, or null when the class closes the fence. */
  suffix: string | null;
  /** The class's indentation in the fence (Java nests it inside `Main`). */
  indent: string;
  /** The fence declared it `static class Solution` and the editor shows `class Solution`. */
  staticKw: boolean;
}

export interface Split {
  harness: Harness;
  /** The `class Solution` block, as the reader sees and edits it. */
  body: string;
}

type Family = "python" | "java" | "cpp";

function familyOf(language: string): Family | null {
  switch (language.trim().toLowerCase()) {
    case "python":
    case "py":
    case "python3":
      return "python";
    case "java":
      return "java";
    case "cpp":
    case "c++":
      return "cpp";
    default:
      return null;
  }
}

const HEAD: Record<Family, RegExp> = {
  python: /^class Solution\b.*:\s*(#.*)?$/,
  java: /^(\s*)((?:public|private|protected|static|final)\s+)*class Solution\b/,
  cpp: /^(class|struct) Solution\b/,
};

/**
 * Index of the `}` that closes the first `{` at or after `start`, skipping comments, string and
 * char literals and Java text blocks, so a brace inside `"{"` never counts. Null when unbalanced.
 */
function closingBrace(src: string, start: number): number | null {
  let depth = 0;
  let i = start;
  const n = src.length;
  while (i < n) {
    if (src.startsWith("//", i)) {
      const j = src.indexOf("\n", i);
      i = j < 0 ? n : j;
      continue;
    }
    if (src.startsWith("/*", i)) {
      const j = src.indexOf("*/", i + 2);
      i = j < 0 ? n : j + 2;
      continue;
    }
    if (src.startsWith('"""', i)) {
      const j = src.indexOf('"""', i + 3);
      i = j < 0 ? n : j + 3;
      continue;
    }
    const c = src[i]!;
    if (c === '"' || c === "'") {
      let j = i + 1;
      while (j < n && src[j] !== c && src[j] !== "\n") j += src[j] === "\\" ? 2 : 1;
      i = j + 1;
      continue;
    }
    if (c === "{") depth += 1;
    else if (c === "}") {
      depth -= 1;
      if (depth === 0) return i;
    }
    i += 1;
  }
  return null;
}

/** Types every kit's harness declares itself. Their definitions above the class are plumbing. */
const PLATFORM = new Set(["ListNode", "TreeNode", "Node", "Main"]);

/** A type or function declared above `class Solution`, at the class's own nesting level. */
const DECL: Record<Family, RegExp> = {
  python: /^(?:class|def)\s+(\w+)/,
  java: /^(?:(?:public|private|protected|static|final|abstract)\s+)*(?:class|interface|record|enum)\s+(\w+)/,
  cpp: /^(?:class|struct|enum(?:\s+class)?)\s+(\w+)/,
};

/** Lines directly above a declaration that belong to it: comments, decorators, annotations. */
const LEAD: Record<Family, RegExp> = {
  python: /^\s*(#|@)/,
  java: /^\s*(\/\/|@)/,
  cpp: /^\s*\/\//,
};

// Java nests every class of the program inside `Main` as a `static` class. The editor shows them
// as plain top-level classes, and the join puts `static` back on each (column 0 in the buffer).
const JAVA_STATIC = /^((?:(?:public|private|protected|final|abstract)\s+)*)static\s+((?:(?:final|abstract)\s+)*(?:class|interface|record|enum)\s+\w)/gm;
const JAVA_TYPE = /^((?:(?:public|private|protected|final|abstract)\s+)*)((?:class|interface|record|enum)\s+\w)/gm;

/** Exclusive end line of the block that opens on line `k`, or null when it cannot be found. */
function blockEnd(family: Family, lines: string[], k: number, source: string): number | null {
  if (family === "python") {
    let end = k + 1;
    while (end < lines.length && (lines[end]!.trim() === "" || /^[ \t]/.test(lines[end]!))) end += 1;
    while (end > k + 1 && lines[end - 1]!.trim() === "") end -= 1;
    return end;
  }
  const offset = lines.slice(0, k).reduce((sum, line) => sum + line.length + 1, 0);
  const brace = source.indexOf("{", offset);
  const semi = source.indexOf(";", offset);
  if (brace < 0 || (semi >= 0 && semi < brace)) return null; // a forward declaration, not a body
  const close = closingBrace(source, offset);
  if (close === null) return null;
  const closeLine = source.slice(0, close).split("\n").length - 1;
  const lineStart = source.lastIndexOf("\n", close - 1) + 1;
  const after = lines[closeLine]!.slice(close - lineStart + 1).trim();
  if (after !== "" && after !== ";") return null; // something else shares the closing line
  return closeLine + 1;
}

const dedent = (block: string[], indent: string): string =>
  block.map((line) => (line.startsWith(indent) ? line.slice(indent.length) : line.trimStart())).join("\n");

/** Split a problem fence into its harness and the visible code: `class Solution`, and any helper
 *  type or function the solution declares above it (a `Cell`, a comparator). Imports and the
 *  platform's node classes stay hidden. Null when the fence has no single `class Solution`, or
 *  nothing around it to hide. */
export function splitHarness(language: string, source: string): Split | null {
  const family = familyOf(language);
  if (family === null) return null;
  const lines = source.split("\n");
  const heads = lines.flatMap((line, k) => (HEAD[family].test(line) ? [k] : []));
  if (heads.length !== 1) return null;
  const head = heads[0]!;
  const indent = family === "python" ? "" : /^\s*/.exec(lines[head]!)![0];
  const end = blockEnd(family, lines, head, source);
  if (end === null) return null;

  const raw = lines.slice(head, end);
  if (raw.some((line) => line.trim() !== "" && !line.startsWith(indent))) return null;

  // Helpers above the class are the solution's own code, so they are shown with it.
  const kept = lines.slice(0, head).map(() => true);
  const helpers: string[] = [];
  for (let k = 0; k < head; k += 1) {
    const line = lines[k]!;
    if (!line.startsWith(indent) || /^\s/.test(line.slice(indent.length))) continue;
    const name = DECL[family].exec(line.slice(indent.length))?.[1];
    if (name === undefined || PLATFORM.has(name)) continue;
    const stop = blockEnd(family, lines, k, source);
    if (stop === null || stop > head) continue;
    let from = k;
    while (from > 0 && kept[from - 1] && lines[from - 1]!.startsWith(indent) && LEAD[family].test(lines[from - 1]!)) from -= 1;
    for (let i = from; i < stop; i += 1) kept[i] = false;
    helpers.push(dedent(lines.slice(from, stop), indent));
    k = stop - 1;
  }

  let body = [...helpers, dedent(raw, indent)].join("\n\n");
  const before = lines.slice(0, head).filter((_, k) => kept[k]);
  if (helpers.length > 0) while (before.length > 0 && before.at(-1)!.trim() === "") before.pop();
  const prefix = before.length > 0 ? before.join("\n") : null;
  const suffix = end < lines.length ? lines.slice(end).join("\n") : null;
  if ((prefix ?? "").trim() === "" && (suffix ?? "").trim() === "") return null; // nothing to hide

  let staticKw = false;
  if (family === "java") {
    staticKw = /^\s*(?:(?:public|private|protected|final|abstract)\s+)*static\s/.test(lines[head]!);
    body = body.replace(JAVA_STATIC, "$1$2");
  }
  return { harness: { prefix, suffix, indent, staticKw }, body };
}

/**
 * The program a runner receives: the reader's class put back into the harness. No harness, or a
 * buffer that is already a whole program (a past submission or a paste that carries its own
 * driver), passes through unchanged — wrapping a program in a second driver would break it.
 */
export function joinHarness(language: string, harness: Harness | null, body: string): string {
  if (harness === null || isWholeProgram(language, body)) return body;
  let code = body;
  if (harness.staticKw) code = code.replace(JAVA_TYPE, "$1static $2");
  if (harness.indent !== "") {
    code = code
      .split("\n")
      .map((line) => (line.trim() === "" ? line : harness.indent + line))
      .join("\n");
  }
  return [harness.prefix, code, harness.suffix].filter((part) => part !== null).join("\n");
}

/**
 * Does `code` carry its own driver? Only code AFTER the class counts: an import the reader adds
 * above `class Solution` is still just their class, and must still be joined into the harness.
 */
function isWholeProgram(language: string, code: string): boolean {
  return (splitHarness(language, code)?.harness.suffix ?? "").trim() !== "";
}

/** What the editor should show for code arriving from outside (Copy to editor, a past
 *  submission): the class alone when the code is a whole program, else the code as it is. */
export function visiblePart(language: string, code: string): string {
  return isWholeProgram(language, code) ? splitHarness(language, code)!.body : code;
}
