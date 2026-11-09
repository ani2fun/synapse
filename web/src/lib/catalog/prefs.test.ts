// The round trip, per-field degradation on a malformed stored string, and the 4-part string
// stored before the Scrolling preference existed.

import { describe, expect, it } from "vitest";
import { DEFAULT_PREFS, parse, serialize } from "./prefs";

describe("prefs", () => {
  it("roundTripsAndDegradesPerField", () => {
    const prefs = { size: "lg", leading: "tight", family: "mono", width: "wide", scroll: "smooth" };
    const stored = serialize(prefs);
    expect(stored).toBe("lg|tight|mono|wide|smooth");
    expect(parse(stored)).toEqual(prefs);

    // One bad token degrades ONLY that field.
    const mixed = parse("lg|banana|mono|wide|smooth");
    expect(mixed.size).toBe("lg");
    expect(mixed.leading).toBe("normal");
    expect(mixed.family).toBe("mono");
    expect(mixed.scroll).toBe("smooth");
    expect(parse("lg|tight|mono|wide|banana").scroll).toBe("instant");
  });

  it("aFourPartStringKeepsItsChoicesAndScrollsInstantly", () => {
    // Every reader who saved a preference before Scrolling existed holds this shape.
    expect(parse("lg|tight|mono|wide")).toEqual({
      size: "lg",
      leading: "tight",
      family: "mono",
      width: "wide",
      scroll: "instant",
    });
  });

  it("absentOrMalformedStorageIsTheDefault", () => {
    expect(parse(null)).toEqual(DEFAULT_PREFS);
    expect(parse("")).toEqual(DEFAULT_PREFS);
    expect(parse("only|three|parts")).toEqual(DEFAULT_PREFS);
    expect(parse("way|too|many|parts|here|now")).toEqual(DEFAULT_PREFS);
  });
});
