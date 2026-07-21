// The placeholder decode contract. The markdown pipeline emits
// `<div class="workbench" data-variants="<uri-encoded JSON>">`; the JSON is
// `[{lang, source, viz?}]`. Languages are trimmed, blank-lang variants dropped, and an empty
// list means the block is skipped. URI decoding is the view/island's job (it needs the DOM) —
// this stays pure and native-testable.

import type { TestSpec } from "./judge";

interface RawVariant {
  lang: string;
  source: string;
  viz?: string | null;
}

/** One language rendition of a runnable block. */
export interface Variant {
  language: string;
  source: string;
  /** The fence's `viz=<structure>[:<root>]` hint, raw (parsed on use). */
  viz: string | null;
}

/** Visualise needs a Python or Java variant with a `viz=` hint. */
export function canVisualise(variant: Variant): boolean {
  return variant.viz != null && ["python", "java"].includes(variant.language.toLowerCase());
}

/** Decode the (already URI-decoded) `data-variants` JSON. Malformed or empty → `null` (the block
 *  is skipped, never a crash — authored content must not take the reader down). */
export function parseVariants(json: string): Variant[] | null {
  let raw: RawVariant[];
  try {
    const parsed: unknown = JSON.parse(json);
    if (!Array.isArray(parsed)) return null;
    raw = parsed as RawVariant[];
  } catch {
    return null;
  }
  const variants: Variant[] = raw
    .map((v) => ({
      language: v.lang.trim(),
      source: v.source,
      viz: v.viz ?? null,
    }))
    .filter((v) => v.language !== "");
  return variants.length === 0 ? null : variants;
}

/** Display name for a fence alias. */
export function displayLang(alias: string): string {
  switch (alias.toLowerCase()) {
    case "cpp":
    case "c++":
      return "C++";
    case "csharp":
      return "C#";
    case "rs":
      return "Rust";
    case "kt":
      return "Kotlin";
    case "js":
      return "JavaScript";
    case "ts":
      return "TypeScript";
    default: {
      const other = alias.toLowerCase();
      return other === "" ? "" : other[0].toUpperCase() + other.slice(1);
    }
  }
}

/** Seed the values grid from an authored case. */
export function seedValues(spec: TestSpec, caseIndex: number): Record<string, string> {
  return spec.cases[caseIndex]?.args ?? {};
}

/** The active case's expected stdout, when declared. */
export function expectedFor(spec: TestSpec, caseIndex: number): string | null {
  return spec.cases[caseIndex]?.expected ?? null;
}

/**
 * Can a judged failure's input be reproduced in the VISIBLE tests panel? Only when every
 * declared arg has a value in the failure.
 *
 * A problem may be judged against a `<stem>.tests.json` sidecar the learner never sees, whose
 * arg ids need not match the authored fence's. Copying misaligned args would leave values under
 * keys with no input field, and `stdinFor` — which iterates the VISIBLE args — would then feed
 * the program something the judge never fed it. Extra keys are harmless: `stdinFor` ignores
 * them.
 */
export function canReproduce(spec: TestSpec, args: Record<string, string>): boolean {
  return spec.args.every((arg) => arg.id in args);
}
