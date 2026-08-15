// The rejection payloads are verbatim mermaid 11.16 output — a hand-written approximation would
// only pin what someone assumed the shape was, and the shape is the whole point of the module.
import { describe, expect, it } from "vitest";

import { authorLineFor, firstProblem } from "./mermaidErrors";

/** What mermaid's jison parsers throw: an `Error` whose message repeats a ZERO-based `hash.line`
 *  as a one-based one, with the lexer's caret art in between. */
function jisonError(message: string, line: number): Error {
  const error = new Error(message);
  (error as Error & { hash: unknown }).hash = {
    text: "",
    token: "SEMI",
    line,
    loc: { first_line: line + 1, last_line: line + 1, first_column: 17, last_column: 18 },
    expected: ["'NODE_STRING'", "'SPACE'"],
  };
  return error;
}

const PARSE_ERROR = [
  "Parse error on line 2:",
  "graph TD;  A --> ;",
  "-----------------^",
  "Expecting 'NODE_STRING', 'SPACE', 'BRKT', 'PS', 'SQS', 'DIAMOND_START', 'TAGEND', got 'SEMI'",
].join("\n");

describe("what mermaid says when it cannot parse", () => {
  it("takes the position from the hash, one-based", () => {
    // The hash counts from zero and the message from one; both name line 2, and the gutter does
    // too, so anything else here sends the caret to the wrong row.
    expect(firstProblem(jisonError(PARSE_ERROR, 1)).line).toBe(2);
  });

  it("falls back to the printed line when there is no hash", () => {
    expect(firstProblem(new Error(PARSE_ERROR)).line).toBe(2);
  });

  it("keeps the verdict and drops the caret art", () => {
    const { message } = firstProblem(jisonError(PARSE_ERROR, 1));
    expect(message).not.toContain("^");
    expect(message).not.toContain("graph TD;");
    expect(message).toContain("got 'SEMI'");
  });

  it("caps the token census — a strip cannot hold forty", () => {
    const { message } = firstProblem(jisonError(PARSE_ERROR, 1));
    expect(message).toBe("Expecting 'NODE_STRING', 'SPACE', 'BRKT'… — got 'SEMI'");
  });

  it("carries a one-line verdict through unchanged", () => {
    const error = jisonError("Parse error on line 4: Unexpected end of input", 3);
    expect(firstProblem(error)).toEqual({ line: 4, message: "Unexpected end of input" });
  });
});

// mermaid lexes a PREPROCESSED copy — frontmatter, `%%{init}%%` directives and `%%` comments are
// gone before line 1 exists — so a reported line is not a gutter line. This is the translation,
// and it is what stops "Go to line N" landing confidently on the wrong row.
describe("translating the parser's line back into the author's buffer", () => {
  it("is the identity when nothing was stripped", () => {
    expect(authorLineFor("flowchart LR\n  A --> B\n  B --> C\n", 2)).toBe(2);
  });

  it("counts past a comment above the mistake", () => {
    // The starter diagram's exact shape: one `%%` line, so everything below reports one high.
    const source = "%% a note\nflowchart LR\n  A --> B\n  B --> ;\n";
    expect(authorLineFor(source, 3)).toBe(4);
  });

  it("counts past comments interleaved with the diagram", () => {
    const source = "%% one\nflowchart LR\n%% two\n  A --> B\n%% three\n  B --> C\n";
    expect(authorLineFor(source, 1)).toBe(2);
    expect(authorLineFor(source, 2)).toBe(4);
    expect(authorLineFor(source, 3)).toBe(6);
  });

  it("counts past a frontmatter block and the blank lines under it", () => {
    const source = "---\ntitle: A diagram\n---\n\nflowchart LR\n  A --> B\n";
    expect(authorLineFor(source, 1)).toBe(5);
    expect(authorLineFor(source, 2)).toBe(6);
  });

  it("counts past an init directive", () => {
    const source = "%%{init: {'theme':'base'}}%%\nflowchart LR\n  A --> B\n";
    expect(authorLineFor(source, 2)).toBe(3);
  });

  it("offers no line rather than a guess when the frontmatter never closes", () => {
    // mermaid's regex does not match an unterminated block, so it strips nothing — and rather
    // than encode which way that falls, this declines to place the caret at all.
    expect(authorLineFor("---\ntitle: oops\nflowchart LR\n", 1)).toBeNull();
  });

  it("offers no line when the parser names one past the end", () => {
    expect(authorLineFor("flowchart LR\n  A --> B\n", 9)).toBeNull();
  });

  it("is what firstProblem reports when handed the source", () => {
    const source = "%% a note\nflowchart LR\n  A --> B\n  B --> ;\n";
    // hash.line 2 → the parser's line 3 → the buffer's line 4.
    expect(firstProblem(jisonError(PARSE_ERROR, 2), source).line).toBe(4);
    // …and without the source it stays the parser's own count, unmapped.
    expect(firstProblem(jisonError(PARSE_ERROR, 2)).line).toBe(3);
  });
});

describe("the errors that carry no position", () => {
  it("does not echo the whole diagram back when no type was detected", () => {
    // mermaid's UnknownDiagramError interpolates the ENTIRE source into its message; pasting a
    // 40-line fence into the error strip buries the sentence that matters.
    const source = "notADiagram\n  A --> B\n  B --> C\n";
    const error = new Error(
      `No diagram type detected matching given configuration for text: ${source}`,
    );
    const problem = firstProblem(error);
    expect(problem.line).toBeNull();
    expect(problem.message).not.toContain("A --> B");
    expect(problem.message).toContain("No diagram type detected");
  });

  it("reports whatever it was given rather than an empty strip", () => {
    expect(firstProblem("something went wrong")).toEqual({
      line: null,
      message: "something went wrong",
    });
  });

  it("survives a hash that a future mermaid spells differently", () => {
    const error = new Error(PARSE_ERROR);
    (error as Error & { hash: unknown }).hash = { line: "two" };
    // The hash is unusable, so the printed line stands in — degrade, never throw.
    expect(firstProblem(error).line).toBe(2);
  });

  it("prefers mermaid's own `str` when it wraps the error", () => {
    const detailed = { str: "Parse error on line 7: Unexpected 'EOF'", hash: { line: 6 } };
    expect(firstProblem(detailed)).toEqual({ line: 7, message: "Unexpected 'EOF'" });
  });
});

// mermaid does not lex the text it is handed: `preprocessDiagram` strips the frontmatter block,
// then `%%{init: …}%%` directives, then every `%%` comment, and only then does the lexer start
// counting. So a reported line is not a gutter line, and one comment above the mistake is enough
// to send "Go to line N" one row high.
describe("translating a parser line back into the author's buffer", () => {
  const at = (source: string, line: number) => firstProblem(jisonError(PARSE_ERROR, line - 1), source).line;

  it("is the identity when nothing was stripped", () => {
    expect(at("flowchart LR\n  a --> b\n  b --> c\n", 3)).toBe(3);
  });

  it("counts past a comment above the mistake", () => {
    // The bug this pins: the starter itself opens with a `%%` comment, so the very first thing an
    // author sees would have jumped one line high.
    const source = "%% a note\nflowchart LR\n  a --> b\n";
    expect(at(source, 2)).toBe(3);
  });

  it("counts past a frontmatter block", () => {
    const source = "---\ntitle: A diagram\n---\nflowchart LR\n  a --> b\n";
    expect(at(source, 2)).toBe(5);
  });

  it("counts past frontmatter, a directive and a comment together", () => {
    const source = [
      "---", // 1
      "title: A diagram", // 2
      "---", // 3
      "%%{init: {'theme':'forest'}}%%", // 4
      "%% a note", // 5
      "flowchart LR", // 6  ← parser line 1
      "  a --> b", // 7  ← parser line 2
      "",
    ].join("\n");
    expect(at(source, 1)).toBe(6);
    expect(at(source, 2)).toBe(7);
  });

  it("counts past the blank lines mermaid's trimStart removes", () => {
    expect(at("\n\nflowchart LR\n  a --> b\n", 2)).toBe(4);
  });

  it("offers no jump rather than a guess when the line is past the end", () => {
    expect(at("flowchart LR\n", 9)).toBeNull();
  });

  it("offers no jump when the frontmatter never closes", () => {
    // mermaid's regex does not match it either, so which lines it stripped is unknowable.
    expect(at("---\ntitle: dangling\nflowchart LR\n", 1)).toBeNull();
  });

  it("reports the parser's own line when no source is given", () => {
    expect(firstProblem(jisonError(PARSE_ERROR, 1)).line).toBe(2);
  });

  it("maps every surviving line of a mixed buffer, in order", () => {
    const source = ["%% one", "flowchart LR", "%% two", "  a --> b", "  b --> c", ""].join("\n");
    // Parser lines 1..3 are the three lines that survive the rewrite, at gutter rows 2, 4 and 5.
    expect([1, 2, 3].map((line) => authorLineFor(source, line))).toEqual([2, 4, 5]);
  });
});
