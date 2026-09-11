// The pure half of the tests draft: the key, the fingerprint, and the envelope. The storage half
// needs a browser and is covered by the Playwright suite.

import { describe, expect, it } from "vitest";
import type { TestSpec } from "../../lib/execution/judge";
import { fingerprintOf, keyFor, parse, serialize } from "./testsDraft";

const USER = "tester";
const PATH = ["dsa", "arrays", "spiral-matrix"];

const AUTHORED: TestSpec = {
  args: [{ id: "matrix", label: "MATRIX", type: "string", placeholder: "[[1, 2], [3, 4]]" }],
  cases: [
    { args: { matrix: "[[1, 2], [3, 4]]" }, expected: "1 2 4 3" },
    { args: { matrix: "[[7]]" }, expected: "7" },
  ],
};
const PRINT = fingerprintOf(AUTHORED);

/** The shape the panel holds after the reader appends a case and types into it. */
const LIVE = [
  ...AUTHORED.cases,
  { args: { matrix: "[[1, 2, 3], [4, 5, 6]]" }, expected: null },
];

describe("testsDraft.keyFor", () => {
  it("isStableForTheSameProblem", () => {
    expect(keyFor(USER, PATH)).toBe(keyFor(USER, PATH));
    expect(keyFor(USER, PATH)).toBe("tests-draft:tester:dsa/arrays/spiral-matrix");
  });

  it("separatesAccountsAndProblems", () => {
    const base = keyFor(USER, PATH);
    // One account's scratch inputs must never surface under another's.
    expect(keyFor("someone-else", PATH)).not.toBe(base);
    // Two problems in one browser are two drafts.
    expect(keyFor(USER, ["dsa", "arrays", "rotate-matrix"])).not.toBe(base);
  });

  it("givesAnonymousItsOwnNamespace", () => {
    // A name no Keycloak handle can be, so a signed-out draft is never adopted on sign-in.
    expect(keyFor(null, PATH)).toBe("tests-draft:@anon:dsa/arrays/spiral-matrix");
    expect(keyFor(null, PATH)).not.toBe(keyFor(USER, PATH));
  });
});

describe("testsDraft.fingerprintOf", () => {
  it("shiftsWhenTheAuthoredSuiteDoes", () => {
    expect(fingerprintOf(AUTHORED)).toBe(PRINT);
    // THE staleness gate: an author adding a case retires every draft that predates it.
    const grown: TestSpec = { ...AUTHORED, cases: [...AUTHORED.cases, { args: { matrix: "[]" } }] };
    expect(fingerprintOf(grown)).not.toBe(PRINT);
    // An edited expected counts too — the reader's ✓ would otherwise be judged against a rule
    // that has since changed.
    const rejudged: TestSpec = {
      ...AUTHORED,
      cases: [{ ...AUTHORED.cases[0]!, expected: "1 2 4 3 " }, AUTHORED.cases[1]!],
    };
    expect(fingerprintOf(rejudged)).not.toBe(PRINT);
  });
});

describe("testsDraft.parse", () => {
  it("roundTripsThroughSerialize", () => {
    expect(parse(serialize(LIVE, 2, PRINT), PRINT)).toEqual({ cases: LIVE, activeCase: 2 });
  });

  it("dropsADraftWrittenAgainstADifferentSuite", () => {
    expect(parse(serialize(LIVE, 2, "deadbeef"), PRINT)).toBeNull();
  });

  it("clampsAChipThatNoLongerExists", () => {
    // The author shortened the suite: the cases are still worth restoring, the selection is not.
    expect(parse(serialize(LIVE, 9, PRINT), PRINT)?.activeCase).toBe(2);
    expect(parse(serialize(LIVE, -1, PRINT), PRINT)?.activeCase).toBe(0);
  });

  it("readsAbsentAndCorruptAsNull", () => {
    expect(parse(null, PRINT)).toBeNull();
    expect(parse("{not json", PRINT)).toBeNull();
    expect(parse("null", PRINT)).toBeNull();
    expect(parse(JSON.stringify({ cases: [], activeCase: 0, authored: PRINT }), PRINT)).toBeNull();
  });

  it("rejectsArgsThatWouldReachStdinAsNonStrings", () => {
    // `stdinFor` writes one line per arg — a non-string would feed the program "[object Object]".
    const bad = JSON.stringify({
      cases: [{ args: { matrix: { nested: true } } }],
      activeCase: 0,
      authored: PRINT,
      savedAt: 1,
    });
    expect(parse(bad, PRINT)).toBeNull();
  });

  it("keepsAnAppendedCasesMissingExpectedAsNull", () => {
    // A case the reader added has nothing to check against, and that must survive the round trip
    // as `null` rather than becoming the string "null" or vanishing.
    const restored = parse(serialize(LIVE, 2, PRINT), PRINT);
    expect(restored?.cases[2]?.expected).toBeNull();
  });
});
