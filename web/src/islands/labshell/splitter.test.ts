import { describe, expect, it } from "vitest";

import { MAX_LEFT_PCT, MIN_LEFT_PCT } from "../../lib/catalog/pane";
import { MAX_TOP_PCT, MIN_TOP_PCT, clampLeftPct } from "./splitter";

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

  it("a row seam travels further than a column seam", () => {
    expect(clampLeftPct(20, 50, "y")).toBe(20);
    expect(clampLeftPct(2, 50, "y")).toBe(MIN_TOP_PCT);
    expect(clampLeftPct(99, 50, "y")).toBe(MAX_TOP_PCT);
    // The same 20% a row keeps is below a column's floor.
    expect(clampLeftPct(20, 50, "x")).toBe(MIN_LEFT_PCT);
  });
});
