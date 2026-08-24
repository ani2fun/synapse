// The pure half of the codebench draft: the key, and the envelope. The storage half needs a
// browser and is covered by the Playwright suite.

import { describe, expect, it } from "vitest";
import { keyFor, parse, serialize } from "./codebenchDraft";

const USER = "tester";
const PATH = "/synapse/java/basics/loops";
const SOURCE = "class Main { public static void main(String[] a) {} }";

describe("codebenchDraft.keyFor", () => {
  it("isStableForTheSameFence", () => {
    expect(keyFor(USER, PATH, "java", SOURCE)).toBe(keyFor(USER, PATH, "java", SOURCE));
    expect(keyFor(USER, PATH, "java", SOURCE)).toMatch(/^codebench-draft:tester:/);
  });

  it("separatesEveryTerm", () => {
    const base = keyFor(USER, PATH, "java", SOURCE);
    // One account's scratch code must never surface under another's.
    expect(keyFor("someone-else", PATH, "java", SOURCE)).not.toBe(base);
    // The same snippet on two pages is two drafts.
    expect(keyFor(USER, "/synapse/java/basics/arrays", "java", SOURCE)).not.toBe(base);
    // A fence group's tabs are separate benches.
    expect(keyFor(USER, PATH, "kotlin", SOURCE)).not.toBe(base);
    // THE staleness gate: the author rewriting the fence retires the draft by itself.
    expect(keyFor(USER, PATH, "java", `${SOURCE}\n// one more line`)).not.toBe(base);
  });
});

describe("codebenchDraft.parse", () => {
  it("roundTripsThroughSerialize", () => {
    const draft = { code: "print(1)", stdin: "5\n12\n", savedAt: 1_756_000_000_000 };
    expect(parse(serialize(draft))).toEqual(draft);
  });

  it("keepsAnEmptyStdinDistinctFromAMissingOne", () => {
    expect(parse(serialize({ code: "x", stdin: "", savedAt: 1 }))?.stdin).toBe("");
    expect(parse(JSON.stringify({ code: "x", savedAt: 1 }))).toBeNull();
  });

  it("readsAnythingUntrustworthyAsNull", () => {
    expect(parse(null)).toBeNull();
    expect(parse("")).toBeNull();
    expect(parse("{ not json")).toBeNull();
    expect(parse("null")).toBeNull();
    // Right shape, wrong types — a hand-edited or half-migrated entry.
    expect(parse(JSON.stringify({ code: 42, stdin: "", savedAt: 1 }))).toBeNull();
    expect(parse(JSON.stringify({ code: "x", stdin: "", savedAt: "yesterday" }))).toBeNull();
  });
});
