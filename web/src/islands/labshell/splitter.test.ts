import { describe, expect, it } from "vitest";

import { MAX_LEFT_PCT, MIN_LEFT_PCT } from "../../lib/catalog/pane";
import { clampLeftPct } from "./splitter";

describe("clampLeftPct", () => {
  it("keeps a width inside the splitter's travel", () => {
    expect(clampLeftPct(50, 44)).toBe(50);
    expect(clampLeftPct(5, 44)).toBe(MIN_LEFT_PCT);
    expect(clampLeftPct(95, 44)).toBe(MAX_LEFT_PCT);
  });

  it("falls back to the caller's default for anything unreadable", () => {
    // Number(null) and Number("") are both 0 — an absent key must not read as a zero-width pane.
    expect(clampLeftPct(Number(null), 44)).toBe(44);
    expect(clampLeftPct(Number(""), 46)).toBe(46);
    expect(clampLeftPct(Number("editorial|52.50|Solution"), 44)).toBe(44);
  });
});
