/**
 * The Format button: a WHITESPACE tidy, the same for all eleven languages.
 *
 * It is not a formatter. Real ones (black, gofmt, rustfmt, prettier…) are a binary per language
 * that the browser does not have and the sandbox does not ship, so this touches only what no
 * language gives meaning to and what every style guide agrees on. It never moves a token: a tidy
 * that rewrote code could change what the program does, and the reader would have no way to tell.
 *
 * It is idempotent — `tidy(tidy(s)) === tidy(s)` — because the page toasts "Already tidy." when
 * nothing changed, and a second click that changed something again would make that a lie.
 */

/**
 * Tidy `source` for `language` (a canonical id: `python`, `go`, …), with `tabSize` the editor's
 * indent width: line endings to LF, trailing whitespace off, blank runs capped (one for Go, as
 * `gofmt` does; two elsewhere, PEP 8's gap between top-level definitions), leading tabs expanded
 * where that cannot change the program, and exactly one final newline.
 *
 * Whitespace INSIDE a line is never touched — a tab there may sit in a string literal. The one
 * place this can reach a literal is a multi-line string (a Python triple quote, a Java text
 * block) whose lines end in spaces or run blank; Format is a button the reader presses, and it
 * lands as one undoable edit.
 */
export function tidy(source: string, language: string, tabSize: number): string {
  const lines = source.replace(/\r\n?/g, "\n").split("\n").map((line) => line.replace(/[ \t]+$/, ""));
  const expand = expandsIndent(lines, language);
  const maxBlank = language === "go" ? 1 : 2;

  const out: string[] = [];
  let blank = 0;
  for (const line of lines) {
    if (line === "") {
      blank += 1;
      continue;
    }
    // Blank lines count only BETWEEN code: leading ones are dropped, a run is capped.
    if (out.length > 0) for (let i = 0; i < Math.min(blank, maxBlank); i += 1) out.push("");
    blank = 0;
    out.push(expand ? expandLeadingTabs(line, tabSize) : line);
  }
  // Empty in, empty out: a file with nothing in it gains no newline to say so.
  return out.length === 0 ? "" : `${out.join("\n")}\n`;
}

/** The languages whose INDENTATION is syntax: a re-nested line is a different program. */
const INDENT_SIGNIFICANT = new Set(["python", "scala"]);

/**
 * Whether leading tabs become spaces. Go keeps them, since `gofmt` indents with tabs. A language
 * whose indentation is syntax keeps them too when the file indents some lines with tabs and others
 * with spaces: the nesting then depends on a tab width nobody wrote down, and any width this picks
 * could silently move a line into or out of a block. Blank lines carry no indentation and are not
 * counted, so the answer is the same on a second pass.
 */
function expandsIndent(lines: readonly string[], language: string): boolean {
  if (language === "go") return false;
  if (!INDENT_SIGNIFICANT.has(language)) return true;
  let tabs = false;
  let spaces = false;
  for (const line of lines) {
    const indent = /^[ \t]*/.exec(line)![0];
    if (indent.length === line.length) continue;
    if (indent.includes("\t")) tabs = true;
    if (indent.includes(" ")) spaces = true;
  }
  return !(tabs && spaces);
}

/** Leading tabs to spaces, each to the NEXT tab stop, so `"  \t"` lands where the editor drew it. */
function expandLeadingTabs(line: string, tabSize: number): string {
  const indent = /^[ \t]*/.exec(line)![0];
  if (!indent.includes("\t")) return line;
  let column = 0;
  for (const ch of indent) column = ch === "\t" ? column + tabSize - (column % tabSize) : column + 1;
  return " ".repeat(column) + line.slice(indent.length);
}
