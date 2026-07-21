// Parity tests for language.ts (oracle: client/src/execution/logic/language.rs — all 8 cases,
// same fixtures, same assertions, case names ported to camelCase).

import { describe, expect, it } from "vitest";
import { LANGUAGES, canonicalLang, preferredIndex, runnableFence } from "./language";
import type { Variant } from "./blocks";

function variant(language: string): Variant {
  return { language, source: "", viz: null };
}

describe("language", () => {
  it("everySpellingOfALanguageFoldsOntoOneToken", () => {
    for (const alias of ["python", "py", "python3", "PYTHON", "  Python  ", "Py"]) {
      expect(canonicalLang(alias), alias).toBe("python");
    }
    expect(canonicalLang("c++")).toBe("cpp");
    expect(canonicalLang("NODE")).toBe("javascript");
  });

  // The alias the client's old flat table was missing — the server has run it since step 09, so
  // assert the fix rather than trusting the table was copied correctly.
  it("sqliteResolvesToSql", () => {
    expect(canonicalLang("sqlite")).toBe("sql");
    expect(canonicalLang("sql")).toBe("sql");
  });

  it("unknownAndBlankAliasesAreNone", () => {
    expect(canonicalLang("cobol")).toBeNull();
    expect(canonicalLang("")).toBeNull();
    expect(canonicalLang("   ")).toBeNull();
    expect(canonicalLang("plaintext")).toBeNull();
  });

  // Ported from the server's `aliases_are_globally_unique_and_round_trip` — an alias claimed by
  // two languages would make the preference resolve differently depending on table order.
  it("aliasesAreGloballyUniqueAndRoundTrip", () => {
    const seen: string[] = [];
    for (const [canonical, aliases] of LANGUAGES) {
      expect(aliases[0], canonical).toBe(canonical);
      for (const alias of aliases) {
        expect(seen.includes(alias), `duplicate alias: ${alias}`).toBe(false);
        seen.push(alias);
        expect(canonicalLang(alias)).toBe(canonical);
      }
    }
  });

  it("preferredIndexFindsTheWantedVariant", () => {
    const variants = [variant("python"), variant("java")];
    expect(preferredIndex(variants, "java")).toBe(1);
    expect(preferredIndex(variants, "python")).toBe(0);
  });

  it("preferredIndexMatchesAcrossAliases", () => {
    let variants = [variant("java"), variant("py")];
    expect(preferredIndex(variants, "python")).toBe(1);
    variants = [variant("java"), variant("python3")];
    expect(preferredIndex(variants, "python")).toBe(1);
  });

  it("preferredIndexFallsBackToTheFirstVariant", () => {
    const variants = [variant("python"), variant("java")];
    expect(preferredIndex(variants, null)).toBe(0);
    expect(preferredIndex(variants, "")).toBe(0);
    expect(preferredIndex(variants, "cobol")).toBe(0);
    // The honest case this whole fallback exists for: a page that simply hasn't got it.
    expect(preferredIndex(variants, "rust")).toBe(0);
  });

  // The "Try in Editor" gate (A09, fenceGroups.ts) — mirrors the server's Language::aliases, so
  // pin the total alongside every alias individually: a drift here means the button appears (or
  // vanishes) for a fence the sandbox can't (or can) actually run.
  it("runnableFenceAcceptsEveryKnownAliasAndRejectsEverythingElse", () => {
    for (const [, aliases] of LANGUAGES) {
      for (const alias of aliases) expect(runnableFence(alias), alias).toBe(true);
    }
    for (const lang of ["bash", "toml", "plaintext", "cobol", "", "   "]) {
      expect(runnableFence(lang), lang).toBe(false);
    }
  });

  it("pinsTheTotalAliasCountAgainstTheServerContract", () => {
    const total = LANGUAGES.reduce((sum, [, aliases]) => sum + aliases.length, 0);
    expect(total).toBe(22);
  });

  // `RunnableBlock`'s island-side indexing does not clamp — this is the invariant that makes
  // that safe, so assert it over the whole cross-product rather than a happy path.
  it("preferredIndexIsAlwaysInBounds", () => {
    const pool = ["python", "java", "rs", "sqlite", "not-a-language", ""];
    for (let len = 1; len <= pool.length; len += 1) {
      const variants = pool.slice(0, len).map((l) => variant(l));
      for (const preference of pool) {
        const i = preferredIndex(variants, preference);
        expect(i < variants.length, `${preference} over ${len} variants → ${i}`).toBe(true);
      }
      expect(preferredIndex(variants, null) < variants.length).toBe(true);
    }
  });
});
