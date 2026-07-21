// The client's fence-alias vocabulary — one table, mirroring
// `server/execution/domain/Language::aliases`. The server stays the authority; an alias added
// there joins this table in the same step.
//
// Two jobs, and the second is why the table exists at all: telling a runnable fence from a
// plaintext one, and folding every spelling of a language onto ONE canonical token so a stored
// preference of `python` still matches a block whose fence says `py` or `python3`.

import type { Variant } from "./blocks";

// `[canonical token, aliases]` — the canonical token is always the first alias, so the stored
// value is the same string the server's `Language::resolve` would land on.
//
// The server keeps its equivalent table module-private (only its own test module reaches it);
// TS has no file-private visibility that would let a separate `*.test.ts` see an unexported
// const, so it is exported here instead (a future language switcher would want this exact list
// too).
export const LANGUAGES: [string, string[]][] = [
  ["python", ["python", "py", "python3"]],
  ["java", ["java"]],
  ["scala", ["scala"]],
  ["c", ["c"]],
  ["cpp", ["cpp", "c++", "cxx"]],
  ["go", ["go", "golang"]],
  ["rust", ["rust", "rs"]],
  ["kotlin", ["kotlin", "kt"]],
  ["typescript", ["typescript", "ts"]],
  ["javascript", ["javascript", "js", "node"]],
  ["sql", ["sql", "sqlite"]],
];

/** Fold a fence alias onto its canonical token: trimmed, case-insensitive; blank or unknown →
 *  `null` (which is also the "this fence is not runnable" answer). */
export function canonicalLang(alias: string): string | null {
  const needle = alias.trim().toLowerCase();
  if (needle === "") return null;
  const found = LANGUAGES.find(([, aliases]) => aliases.includes(needle));
  return found ? found[0] : null;
}

/** A fence the engine can actually run — a thin delegation to `canonicalLang`. The "Try in
 *  Editor" button (fenceGroups.ts) gates on this, so the alias table and the fence-eligibility
 *  check share the ONE list and cannot drift apart. */
export function runnableFence(lang: string): boolean {
  return canonicalLang(lang) !== null;
}

/**
 * Which variant a block should open on, given the reader's stored language preference.
 *
 * Falls back to 0 whenever the preference cannot be honoured — absent, blank, a language this
 * build doesn't know, or simply not among THIS block's variants. Built from `findIndex`, so the
 * result is structurally in-bounds for any inputs; callers index `variants` with it directly.
 */
export function preferredIndex(variants: Variant[], preferred: string | null | undefined): number {
  const wanted = preferred == null ? null : canonicalLang(preferred);
  if (wanted === null) return 0;
  const index = variants.findIndex((v) => canonicalLang(v.language) === wanted);
  return index === -1 ? 0 : index;
}
