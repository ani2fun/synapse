// Parity tests for judge.ts against the shared cross-language vector file — the Rust twin
// (shared/src/execution/judge_vectors_test.rs) runs the SAME `shared/test-vectors/judge-vectors
// .json`, so a rule change on either side that isn't mirrored on the other fails a test here or
// there.

import { describe, expect, it } from "vitest";
import vectors from "../../../../shared/test-vectors/judge-vectors.json";
import { judge, pendingPrompt, ranOutOfInput, stdinFor } from "./judge";
import type { ArgSpec, Verdict } from "./judge";
import type { components } from "../api/schema.gen";

type RunStatus = components["schemas"]["RunStatus"];
type RunResult = components["schemas"]["RunResult"];

interface JudgeVector {
  name: string;
  status: RunStatus;
  stdout: string;
  expected: string | null;
  verdict: Verdict;
}

interface StdinVector {
  name: string;
  argIds: string[];
  values: Record<string, string>;
  expected: string;
}

function result(status: RunStatus, stdout: string): RunResult {
  return { status, stdout, stderr: "", compileOutput: "", timeSeconds: null, memoryKb: null };
}

function argSpecs(ids: string[]): ArgSpec[] {
  return ids.map((id) => ({ id, label: id, type: "text", placeholder: null }));
}

describe("judge (cross-language vectors)", () => {
  for (const v of vectors.judgeVectors as JudgeVector[]) {
    it(`judge: ${v.name}`, () => {
      expect(judge(result(v.status, v.stdout), v.expected)).toBe(v.verdict);
    });
  }

  for (const v of vectors.stdinVectors as StdinVector[]) {
    it(`stdinFor: ${v.name}`, () => {
      expect(stdinFor(argSpecs(v.argIds), v.values)).toBe(v.expected);
    });
  }
});

// ── running out of input ──────────────────────────────────────────────────────

describe("ranOutOfInput", () => {
  const died = (stderr: string): RunResult => ({
    status: "RuntimeError",
    stdout: "",
    stderr,
    compileOutput: "",
    timeSeconds: null,
    memoryKb: null,
  });

  it("recognises a Python program that read past the end of stdin", () => {
    expect(ranOutOfInput(died("EOFError: EOF when reading a line"))).toBe(true);
  });

  it("recognises Java's Scanner running dry", () => {
    expect(
      ranOutOfInput(
        died("java.util.NoSuchElementException\n\tat java.util.Scanner.nextLine(Scanner.java:1651)"),
      ),
    ).toBe(true);
  });

  it("leaves an empty-collection NoSuchElementException alone", () => {
    // A different bug entirely. Renaming it would send the reader to their input when the fault
    // is in their code.
    expect(
      ranOutOfInput(died("java.util.NoSuchElementException\n\tat java.util.ArrayDeque.removeFirst")),
    ).toBe(false);
  });

  it("says nothing about a run that simply failed", () => {
    expect(ranOutOfInput(died("ZeroDivisionError: division by zero"))).toBe(false);
    expect(ranOutOfInput(died(""))).toBe(false);
  });
});

describe("pendingPrompt", () => {
  it("quotes the question a program left on its last, unfinished line", () => {
    // `input("How many? ")` writes the question and no newline, then reads — and finds nothing.
    expect(pendingPrompt("How many? ")).toBe("How many?");
    expect(pendingPrompt("Welcome!\nName: ")).toBe("Name:");
  });

  it("has no question to quote when the output ends at a line break", () => {
    // A bare `input()` after a finished line asks with no words — the host says so itself.
    expect(pendingPrompt("done\n")).toBeNull();
    expect(pendingPrompt("")).toBeNull();
    expect(pendingPrompt("   ")).toBeNull();
  });

  it("quotes only the NEW question on a re-run that answered the last one", () => {
    // Answers are not echoed, so both prompts share a line: read whole, it would quote both.
    const first = "How many numbers? ";
    const second = "How many numbers? Number 1: ";
    expect(pendingPrompt(second)).toBe("How many numbers? Number 1:");
    expect(pendingPrompt(second, first)).toBe("Number 1:");
    expect(pendingPrompt("How many numbers? Number 1: Number 2: ", second)).toBe("Number 2:");
  });

  it("reads a run whole when it is not a continuation of the earlier one", () => {
    // The code changed between runs, so the earlier output is no prefix of this one.
    expect(pendingPrompt("Name: ", "How many numbers? ")).toBe("Name:");
  });

  it("has no question when the re-run asked with no words", () => {
    expect(pendingPrompt("Count: \n", "Count: ")).toBeNull();
  });
});
