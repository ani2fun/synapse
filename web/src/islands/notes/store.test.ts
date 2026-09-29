import { describe, expect, it } from "vitest";

import { NOTES_MAX } from "./format";
import { keyFor, parse, reconcile, serialize } from "./store";
import type { LocalNote } from "./store";

const T1 = "2026-09-29T08:00:00.000Z";
const T2 = "2026-09-29T09:00:00.000Z";

function local(overrides: Partial<LocalNote>): LocalNote {
  return { text: "local", savedAt: Date.parse(T1), synced: false, base: null, ...overrides };
}

describe("notes store", () => {
  it("keys by account and problem, with a namespace no username can take", () => {
    expect(keyFor("ada", ["dsa", "p", "two-sum"])).toBe("problem-notes:ada:dsa/p/two-sum");
    expect(keyFor(null, ["dsa", "p", "two-sum"])).toBe("problem-notes:@anon:dsa/p/two-sum");
  });

  it("round-trips", () => {
    const note = local({ text: "# hi", savedAt: 42, synced: true, base: T1 });
    expect(parse(serialize(note))).toEqual(note);
  });

  it("reads absent, corrupt and wrong-shaped values as nothing", () => {
    expect(parse(null)).toBeNull();
    expect(parse("{not json")).toBeNull();
    expect(parse('"a string"')).toBeNull();
    expect(parse('{"text": 7}')).toBeNull();
  });

  it("reads a note from before sync as never synced, so it gets pushed", () => {
    expect(parse('{"text":"old","savedAt":5}')).toEqual({ text: "old", savedAt: 5, synced: false, base: null });
  });

  it("truncates an over-long note rather than refusing it", () => {
    const long = "x".repeat(NOTES_MAX + 5);
    expect(parse(serialize(local({ text: long })))?.text.length).toBe(NOTES_MAX);
  });
});

describe("reconcile", () => {
  it("adopts the account's copy when this browser has none", () => {
    expect(reconcile(null, { text: "server", updatedAt: T1 })).toEqual({ text: "server", push: false, base: T1 });
  });

  it("adopts the account's copy over a local one it already has — even an emptied one", () => {
    expect(reconcile(local({ synced: true, base: T1 }), { text: "newer", updatedAt: T2 }).text).toBe("newer");
    expect(reconcile(local({ synced: true, base: T1 }), { text: "", updatedAt: null })).toEqual({
      text: "",
      push: false,
      base: null,
    });
  });

  it("pushes unsynced edits when the server has not moved since they began", () => {
    expect(reconcile(local({ base: T1 }), { text: "server", updatedAt: T1 })).toEqual({
      text: "local",
      push: true,
      base: T1,
    });
    // A first note, written before the account had any.
    expect(reconcile(local({ base: null }), { text: "", updatedAt: null }).push).toBe(true);
  });

  it("has nothing to send when unsynced edits already match the server", () => {
    expect(reconcile(local({ text: "same" }), { text: "same", updatedAt: T2 })).toEqual({
      text: "same",
      push: false,
      base: T2,
    });
  });

  it("settles a real conflict in favour of the newer write", () => {
    const older = local({ base: T1, savedAt: Date.parse(T1) + 60_000 });
    expect(reconcile(older, { text: "server", updatedAt: T2 }).text).toBe("server");

    const newer = local({ base: T1, savedAt: Date.parse(T2) + 60_000 });
    expect(reconcile(newer, { text: "server", updatedAt: T2 })).toEqual({ text: "local", push: true, base: T2 });
  });
});
