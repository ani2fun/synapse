import { describe, expect, it } from "vitest";

import { tidy } from "./tidy";

const SAMPLES: [string, string][] = [
  ["python", "def f(x):   \n\tif x:\n\t\treturn 1\n\n\n\n    return 0"],
  ["go", "package main\n\nfunc main() {\n\tprintln(1)  \n}\n\n\n"],
  ["java", "class Main {\r\n  public static void main(String[] a) {}\r\n}"],
  ["sql", ""],
];

describe("tidy", () => {
  it("is idempotent, so a second Format always says 'Already tidy.'", () => {
    for (const [language, source] of SAMPLES) {
      const once = tidy(source, language, 4);
      expect(tidy(once, language, 4)).toBe(once);
    }
  });

  it("never touches what is inside a line", () => {
    const line = 'print("a   b\\t c")';
    expect(tidy(`${line}\n`, "python", 4)).toBe(`${line}\n`);
  });
});

describe("tidy's rules", () => {
  it("turns CRLF and bare CR into LF", () => {
    expect(tidy("a\r\nb\rc", "java", 4)).toBe("a\nb\nc\n");
  });

  it("drops trailing whitespace and ends on exactly one newline", () => {
    expect(tidy("x = 1   \t\ny = 2\n\n\n", "python", 4)).toBe("x = 1\ny = 2\n");
  });

  it("drops leading blank lines and caps a blank run at two, or one for Go", () => {
    expect(tidy("\n\na\n\n\n\n\nb", "python", 4)).toBe("a\n\n\nb\n");
    expect(tidy("a\n\n\n\nb", "go", 4)).toBe("a\n\nb\n");
  });

  it("expands leading tabs to the next tab stop, leaving tabs mid-line alone", () => {
    expect(tidy("\tif (x) {\n  \ty();\t// z\n}", "c", 4)).toBe("    if (x) {\n    y();\t// z\n}\n");
    expect(tidy("\tx", "javascript", 2)).toBe("  x\n");
  });

  it("keeps Go's tabs, because gofmt indents with them", () => {
    expect(tidy("func f() {\n\treturn\n}\n", "go", 4)).toBe("func f() {\n\treturn\n}\n");
  });

  it("expands a Python file indented with tabs alone", () => {
    expect(tidy("def f():\n\tif x:\n\t\treturn 1\n", "python", 4)).toBe(
      "def f():\n    if x:\n        return 1\n",
    );
  });

  it("leaves a Python or Scala file's MIXED indentation alone, since any tab width is a guess", () => {
    const mixed = "def f():\n\tif x:\n        return 1\n";
    expect(tidy(mixed, "python", 4)).toBe(mixed);
    expect(tidy("object A:\n\tdef a = 1\n  def b = 2\n", "scala", 4)).toBe("object A:\n\tdef a = 1\n  def b = 2\n");
  });

  it("does not count a whitespace-only line as indentation", () => {
    // The blank line's stray spaces vanish on the first pass; the answer must not change on the second.
    expect(tidy("def f():\n  \n\treturn 1\n", "python", 4)).toBe("def f():\n\n    return 1\n");
  });

  it("leaves an empty or whitespace-only buffer empty", () => {
    expect(tidy("", "sql", 4)).toBe("");
    expect(tidy("  \n\t\n", "sql", 4)).toBe("");
  });
});
