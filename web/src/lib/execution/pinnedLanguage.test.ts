// The pinned language: what it stores, what it matches, and what it tells the page.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LANGUAGE_PINNED, type LanguagePinned } from "../../islands/workbench/contracts";
import { LANGUAGE_PIN_KEY } from "../storage";
import { isPinned, pinLanguage, pinnedLanguage } from "./pinnedLanguage";

/** A window with just what the module touches: localStorage and an event target. */
function fakeWindow() {
  const items = new Map<string, string>();
  const target = new EventTarget();
  return Object.assign(target, {
    localStorage: {
      getItem: (key: string) => items.get(key) ?? null,
      setItem: (key: string, value: string) => void items.set(key, value),
      removeItem: (key: string) => void items.delete(key),
    },
  });
}

describe("pinnedLanguage", () => {
  beforeEach(() => vi.stubGlobal("window", fakeWindow()));
  afterEach(() => vi.unstubAllGlobals());

  it("nothingIsPinnedAtFirst", () => {
    expect(pinnedLanguage()).toBeNull();
    expect(isPinned("python")).toBe(false);
  });

  it("storesTheCanonicalToken", () => {
    pinLanguage("py");
    expect(pinnedLanguage()).toBe("python");
    expect(window.localStorage.getItem(LANGUAGE_PIN_KEY)).toBe("python");
  });

  it("matchesEverySpellingOfThePinnedLanguage", () => {
    pinLanguage("c++");
    expect(isPinned("cpp")).toBe(true);
    expect(isPinned("cxx")).toBe(true);
    expect(isPinned("java")).toBe(false);
  });

  it("anUnknownLanguageIsNeitherPinnableNorPinned", () => {
    pinLanguage("java");
    pinLanguage("bash");
    expect(pinnedLanguage()).toBe("java");
    expect(isPinned("bash")).toBe(false);
  });

  it("tellsThePageTheCanonicalToken", () => {
    const seen: string[] = [];
    window.addEventListener(LANGUAGE_PINNED, (event) =>
      seen.push((event as CustomEvent<LanguagePinned>).detail.language),
    );
    pinLanguage("python3");
    pinLanguage("bash");
    expect(seen).toEqual(["python"]);
  });
});
