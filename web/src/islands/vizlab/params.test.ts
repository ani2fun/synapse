import { describe, expect, it } from "vitest";

import { composeHint, paramsFromUrl, splitHint } from "./params";

describe("splitHint", () => {
  it("reads a bare structure", () => {
    expect(splitHint("array")).toEqual({ structure: "array", root: null });
  });

  it("reads a dotted root", () => {
    expect(splitHint("list:self.head")).toEqual({ structure: "list", root: "self.head" });
  });

  // A colon with nothing after it declares NO root rather than an empty one — the same reading
  // `VizStructure::parse` gives it, and the two must not disagree about a URL a link produced.
  it("treats a trailing colon as no root", () => {
    expect(splitHint("stack:")).toEqual({ structure: "stack", root: null });
  });

  it("is blank for a blank hint", () => {
    expect(splitHint("   ")).toEqual({ structure: null, root: null });
  });

  // Unknown tokens survive the parse: this module has no vocabulary, by design.
  it("does not judge the token", () => {
    expect(splitHint("btree").structure).toBe("btree");
  });
});

describe("composeHint", () => {
  it("round-trips a hint with a root", () => {
    expect(composeHint("array", "arr")).toBe("array:arr");
    expect(splitHint(composeHint("array", "arr"))).toEqual({ structure: "array", root: "arr" });
  });

  it("omits the colon when there is no root", () => {
    expect(composeHint("graph", null)).toBe("graph");
    expect(composeHint("graph", "  ")).toBe("graph");
  });
});

describe("paramsFromUrl", () => {
  it("reads both params", () => {
    expect(paramsFromUrl("?s=array:arr&lang=Java")).toEqual({
      structure: "array",
      root: "arr",
      language: "java",
    });
  });

  it("is all-null for a bare page", () => {
    expect(paramsFromUrl("")).toEqual({ structure: null, root: null, language: null });
  });
});
