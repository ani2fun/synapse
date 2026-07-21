// Two cases: the round trip, and per-field degradation on a malformed stored string.

import { describe, expect, it } from "vitest";
import { DEFAULT_PREFS, parse, serialize } from "./prefs";

describe("prefs", () => {
  it("roundTripsAndDegradesPerField", () => {
    const stored = serialize({ size: "lg", leading: "tight", family: "mono", width: "wide" });
    expect(stored).toBe("lg|tight|mono|wide");
    const parsed = parse(stored);
    expect(parsed.size).toBe("lg");
    expect(parsed.family).toBe("mono");

    // One bad token degrades ONLY that field.
    const mixed = parse("lg|banana|mono|wide");
    expect(mixed.size).toBe("lg");
    expect(mixed.leading).toBe("normal");
    expect(mixed.family).toBe("mono");
  });

  it("absentOrMalformedStorageIsTheDefault", () => {
    expect(parse(null)).toEqual(DEFAULT_PREFS);
    expect(parse("")).toEqual(DEFAULT_PREFS);
    expect(parse("only|three|parts")).toEqual(DEFAULT_PREFS);
    expect(parse("way|too|many|parts|here")).toEqual(DEFAULT_PREFS);
  });
});
