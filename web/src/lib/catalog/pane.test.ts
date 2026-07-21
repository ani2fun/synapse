// Oracle: client/src/catalog/logic/pane.rs's own `mod tests` — the same six cases, camelCased.

import { describe, expect, it } from "vitest";
import {
  DEFAULT_LEFT_PCT,
  MAX_LEFT_PCT,
  MIN_LEFT_PCT,
  normalizeLabel,
  parseLeftPct,
  sectionIndex,
  serializeLeftPct,
} from "./pane";

describe("pane — the splitter width", () => {
  it("the width round-trips at the precision the view renders", () => {
    expect(serializeLeftPct(52.5)).toBe("52.50");
    expect(parseLeftPct(serializeLeftPct(52.5))).toBeCloseTo(52.5);
    // A raw drag lands on sixteen digits; only two of them are kept.
    expect(serializeLeftPct(55.67703952901598)).toBe("55.68");
  });

  it("the width clamps to the splitter travel", () => {
    expect(parseLeftPct("999")).toBeCloseTo(MAX_LEFT_PCT);
    expect(parseLeftPct("1")).toBeCloseTo(MIN_LEFT_PCT);
  });

  // Includes a step-47 `tab|pct|section` record: unreadable now, and deliberately so — the width
  // resets once rather than the format growing a legacy branch forever.
  it("anything unreadable is the default width", () => {
    for (const stored of [null, "", "banana", "editorial|52.50|Solution"]) {
      expect(parseLeftPct(stored)).toBeCloseTo(DEFAULT_LEFT_PCT);
    }
  });

  it("labels match however they were typed", () => {
    expect(normalizeLabel("Complexity Analysis")).toBe(normalizeLabel("  complexity   analysis "));
    const labels = ["Approach", "Solution", "Complexity Analysis"];
    expect(sectionIndex(labels, "Complexity Analysis")).toBe(2);
    expect(sectionIndex(labels, "  SOLUTION  ")).toBe(1);
  });

  it("an unmatched section falls back to the first", () => {
    const labels = ["Approach", "Solution"];
    expect(sectionIndex(labels, "Proof of correctness")).toBe(0);
    expect(sectionIndex(labels, "")).toBe(0);
    expect(sectionIndex([], "Solution")).toBe(0);
  });

  it("duplicate labels resolve to the first", () => {
    const labels = ["Solution", "Solution"];
    expect(sectionIndex(labels, "Solution")).toBe(0);
  });
});
