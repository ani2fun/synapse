// The shapes d2 actually rejects with, captured from the engine rather than imagined — a
// mis-parse here is what puts a JSON array in front of an author instead of a sentence.
import { describe, expect, it } from "vitest";

import { d2Problems, firstProblem } from "./d2Errors";

/** Verbatim from `@terrastruct/d2` v0.8.1. */
const UNCLOSED = '[{"range":"index,0:7:7-0:8:8","errmsg":"index:1:8: maps must be terminated with }"}]';
const BAD_SHAPE =
  '[{"range":"index,0:12:12-0:20:20","errmsg":"index:1:13: unknown shape \\"nonsense\\""}]';
const TWO =
  '[{"range":"a","errmsg":"index:9:3: connection missing destination"},' +
  '{"range":"b","errmsg":"index:2:1: unknown shape \\"nope\\""}]';

describe("reading d2's compile failures", () => {
  it("splits the position off the sentence", () => {
    expect(d2Problems(new Error(UNCLOSED))).toEqual([
      { line: 1, column: 8, message: "maps must be terminated with }" },
    ]);
    expect(d2Problems(new Error(BAD_SHAPE))).toEqual([
      { line: 1, column: 13, message: 'unknown shape "nonsense"' },
    ]);
  });

  it("reads every problem, not just the first in the array", () => {
    expect(d2Problems(new Error(TWO))).toHaveLength(2);
  });

  it("leads with the EARLIEST line, since later ones are usually its echo", () => {
    expect(firstProblem(new Error(TWO))).toEqual({
      line: 2,
      column: 1,
      message: 'unknown shape "nope"',
    });
  });

  it("takes the array itself, not only an Error wrapping it", () => {
    expect(d2Problems(JSON.parse(UNCLOSED))).toEqual(d2Problems(new Error(UNCLOSED)));
  });

  it("still reports something when the shape is unrecognisable", () => {
    // A d2 upgrade that changes this format must cost the jump affordance, not the error.
    for (const raw of ["boom", "[]", "[{}]", "{", '["no errmsg here"]']) {
      const problems = d2Problems(new Error(raw));
      expect(problems, raw).toHaveLength(1);
      expect(problems[0]!.message, raw).not.toBe("");
      expect(problems[0]!.line, raw).toBeNull();
    }
  });

  it("never returns an empty list, so a failure always has something to say", () => {
    for (const input of [new Error(""), "", null, undefined, 42]) {
      expect(d2Problems(input).length).toBeGreaterThan(0);
    }
  });
});
