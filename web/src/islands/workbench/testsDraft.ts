// The problem page's live test suite. A reader who appends a case with `+`, or types their own
// matrix into an authored one, keeps it — across a chip switch, across a reload, across coming back
// to the problem tomorrow — because a scratchpad that silently empties itself is not one.
//
// INPUTS ONLY. Verdict ticks and the output panel are deliberately absent, and that is not an
// oversight to correct later: the CODE buffer is not persisted on this page — the workbench re-mints
// every store from the authored starter on load — so a restored ✓ would badge a run whose source is
// gone. It is the same lie `ranCase` exists to prevent (state.ts), one reload further out.
//
// Two things make the key. The USERNAME, so a shared browser never shows one account's scratch
// inputs to the next (anonymous drafts get their own namespace and are never adopted on sign-in).
// And the PROBLEM PATH, so the same browser holds a draft per problem.
//
// A FINGERPRINT of the authored suite rides in the payload and retires the draft by itself: when
// the author edits the `.tests.json` sidecar, a stored suite that outlived it would mask the new
// authored cases forever, and nothing on screen would say so. A draft only ever comes back to the
// suite it was started from — the discipline `codebenchDraft.ts` gets from its fence fingerprint.
//
// The pure half (`keyFor`, `fingerprintOf`, `serialize`, `parse`) is separated from the storage half
// so it can be unit-tested: the vitest suite runs in node, with no `localStorage` at all.

import { fnv1a } from "../../lib/hash";
import type { TestCase, TestSpec } from "../../lib/execution/judge";
import * as log from "../../lib/log";
import * as storage from "../../lib/storage";

/** The anonymous namespace. Deliberately a name no Keycloak handle can be, so an anonymous draft
 *  and a signed-in one never collide. */
const ANON = "@anon";

export function keyFor(username: string | null, path: string[]): string {
  return `${storage.TESTS_DRAFT_PREFIX}${username ?? ANON}:${path.join("/")}`;
}

/** The authored suite's fingerprint. Must be taken from the AUTHORED spec — the one the page was
 *  served with — never the live one, or every keystroke would retire the draft it just wrote. */
export function fingerprintOf(authored: TestSpec): string {
  return fnv1a(JSON.stringify(authored));
}

interface StoredDraft {
  cases: TestCase[];
  /** The chip that was selected. Restoring the reader to the case they were looking at is the
   *  natural reading of "where I left off"; it is clamped on the way back in. */
  activeCase: number;
  /** `fingerprintOf` the authored suite this draft was started from. */
  authored: string;
  /** epoch ms — what an expiry sweep would need, one field now versus a migration later. */
  savedAt: number;
}

export interface RestoredDraft {
  cases: TestCase[];
  activeCase: number;
}

export function serialize(cases: TestCase[], activeCase: number, authored: string): string {
  return JSON.stringify({ cases, activeCase, authored, savedAt: Date.now() } satisfies StoredDraft);
}

/** One stored case, or `null` if it is not the shape this module wrote. `args` must be a flat
 *  string map — it is fed to `stdinFor`, which would otherwise hand the program a `[object
 *  Object]` line. */
function parseCase(value: unknown): TestCase | null {
  if (typeof value !== "object" || value === null) return null;
  const raw = value as Partial<TestCase>;
  if (typeof raw.args !== "object" || raw.args === null) return null;
  const args: Record<string, string> = {};
  for (const [id, argValue] of Object.entries(raw.args)) {
    if (typeof argValue !== "string") return null;
    args[id] = argValue;
  }
  if (raw.expected != null && typeof raw.expected !== "string") return null;
  return { args, expected: raw.expected ?? null };
}

/**
 * Absent, unparseable, the wrong shape, or written against a DIFFERENT authored suite all read as
 * `null` — a draft is a convenience, and one that cannot be trusted is worth less than the suite
 * the author shipped.
 *
 * `activeCase` is clamped rather than rejected: a draft written when the suite had more cases is
 * still worth restoring, just not with a chip selected that no longer exists.
 */
export function parse(raw: string | null, authored: string): RestoredDraft | null {
  if (raw === null) return null;
  try {
    const stored = JSON.parse(raw) as Partial<StoredDraft>;
    if (typeof stored !== "object" || stored === null) return null;
    if (stored.authored !== authored) return null;
    if (!Array.isArray(stored.cases) || stored.cases.length === 0) return null;
    const cases: TestCase[] = [];
    for (const value of stored.cases) {
      const testCase = parseCase(value);
      if (testCase === null) return null;
      cases.push(testCase);
    }
    const wanted = typeof stored.activeCase === "number" ? stored.activeCase : 0;
    const activeCase = Math.min(Math.max(Math.trunc(wanted), 0), cases.length - 1);
    return { cases, activeCase };
  } catch {
    return null;
  }
}

/** The saved suite for this problem, if one exists, still parses, and still belongs to the authored
 *  suite on the page. A corrupt entry is dropped on the way past rather than left to fail
 *  identically on every visit; a STALE one (the author moved) is dropped the same way. */
export function load(key: string, authored: string): RestoredDraft | null {
  const raw = storage.get(key);
  const draft = parse(raw, authored);
  if (raw !== null && draft === null) {
    storage.remove(key);
    log.debug("tests: dropped a draft that no longer applies");
  }
  return draft;
}

/** Persist the live suite (debounced by the caller). A denied write is a silent no-op — the
 *  accessor swallows it — so a storage-denied profile degrades to losing the cases on reload rather
 *  than breaking the panel. */
export function save(key: string, cases: TestCase[], activeCase: number, authored: string): void {
  storage.set(key, serialize(cases, activeCase, authored));
}

export function clear(key: string): void {
  storage.remove(key);
}
