// Nine cases covering blocks.ts's variant decode, display-name mapping, and reproducibility check.

import { describe, expect, it } from "vitest";
import type { TestSpec } from "./judge";
import { canReproduce, displayLang, parseVariants } from "./blocks";

function specWith(ids: string[]): TestSpec {
  return {
    args: ids.map((id) => ({ id, label: id, type: "text", placeholder: null })),
    cases: [],
  };
}

function argsWith(pairs: [string, string][]): Record<string, string> {
  return Object.fromEntries(pairs);
}

describe("blocks", () => {
  it("aFailureCoveringEveryDeclaredArgIsReproducible", () => {
    const spec = specWith(["nums", "target"]);
    expect(
      canReproduce(spec, argsWith([["nums", "[1,2]"], ["target", "3"]])),
    ).toBe(true);
  });

  // Extra keys are fine — `stdinFor` iterates the DECLARED args and ignores the rest.
  it("extraKeysInTheFailureDoNotBlockIt", () => {
    const spec = specWith(["nums"]);
    expect(canReproduce(spec, argsWith([["nums", "[1]"], ["k", "9"]]))).toBe(true);
  });

  // The case the guard exists for: a hidden sidecar suite declaring args the fence doesn't.
  it("aMissingDeclaredArgBlocksReproduction", () => {
    const spec = specWith(["nums", "target"]);
    expect(canReproduce(spec, argsWith([["nums", "[1,2]"]]))).toBe(false);
    expect(
      canReproduce(spec, argsWith([["numbers", "[1,2]"], ["target", "3"]])),
    ).toBe(false);
  });

  it("anEmptyFailureCannotCoverADeclaredArg", () => {
    expect(canReproduce(specWith(["nums"]), {})).toBe(false);
  });

  // A stdin-free problem declares nothing, so there is nothing to misalign.
  it("aSpecWithNoArgsIsVacuouslyReproducible", () => {
    expect(canReproduce(specWith([]), {})).toBe(true);
    expect(canReproduce(specWith([]), argsWith([["stray", "1"]]))).toBe(true);
  });

  it("decodesSingleAndAdjacentVariantsInOrder", () => {
    const json =
      '[{"lang":"python","source":"print(1)"},{"lang":"java","source":"class S {}","viz":"array"}]';
    const variants = parseVariants(json);
    expect(variants).not.toBeNull();
    expect(variants?.length).toBe(2);
    expect(variants?.[0].language).toBe("python");
    expect(variants?.[1].source).toBe("class S {}");
  });

  it("trimsLangsAndDropsBlankOnes", () => {
    const json = '[{"lang":"  py  ","source":"a"},{"lang":"   ","source":"b"}]';
    const variants = parseVariants(json);
    expect(variants?.length).toBe(1);
    expect(variants?.[0].language).toBe("py");
  });

  it("malformedOrEmptyMeansSkip", () => {
    expect(parseVariants("not json")).toBeNull();
    expect(parseVariants("[]")).toBeNull();
    expect(parseVariants('[{"lang":" ","source":"x"}]')).toBeNull();
  });

  it("displayNamesReadWell", () => {
    expect(displayLang("cpp")).toBe("C++");
    expect(displayLang("python")).toBe("Python");
    expect(displayLang("rs")).toBe("Rust");
    expect(displayLang("kt")).toBe("Kotlin");
    expect(displayLang("js")).toBe("JavaScript");
  });
});
