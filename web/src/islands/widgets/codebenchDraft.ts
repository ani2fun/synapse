// The codebench's edited buffer. A reader who changes a snippet in the popup keeps that change —
// across closing the modal, across a reload, across coming back to the lesson tomorrow — because
// the alternative is that "Try in Editor" quietly throws away work every time it is closed.
//
// The draft lives in THIS browser's localStorage and never reaches the server: a snippet somebody
// is poking at is not a contribution, and the run endpoint already receives the buffer whenever
// they press Run.
//
// Three things make up the key. The USERNAME, because editing is gated on sign-in and a shared
// browser must not show one account's scratch code to the next. The PAGE PATH, so the same snippet
// on two pages stays two drafts. And a FINGERPRINT OF THE AUTHORED FENCE, which does double duty:
// it separates the fences on one page, and it retires the draft by itself when the author rewrites
// that fence — a draft only ever comes back to the source it was started from, the same discipline
// `authoring/draft.ts` gets from its `baseFingerprint`.
//
// The pure half (`keyFor`, `serialize`, `parse`) is separated from the storage half so it can be
// unit-tested: the vitest suite runs in a node environment with no `localStorage` at all.

import { fnv1a } from "../../lib/hash";
import * as storage from "../../lib/storage";
import * as log from "../../lib/log";

export interface CodebenchDraft {
  readonly code: string;
  /** The stdin box rides along — typing input for a snippet and losing it on reopen is the same
   *  annoyance as losing the code. */
  readonly stdin: string;
  /** epoch ms. Nothing reads it yet; it is what a future expiry sweep would need, and it costs
   *  one field to record now versus a migration to add later. */
  readonly savedAt: number;
}

/**
 * The key for one fence.
 *
 * `source` must be the AUTHORED fence text, never the live buffer — `fenceGroups.collectPanes`
 * reads `pre.textContent` once at hydration and hands the same string to every open, so the
 * fingerprint is stable while the reader types and shifts only when the LESSON changes.
 *
 * `pathname` is passed in rather than read here: it keeps the function pure, and the caller uses
 * `window.location.pathname` rather than `lessonPathFromUrl()` because the codebench also mounts
 * on blog and problem pages, where that helper resolves to nothing.
 */
export function keyFor(username: string, pathname: string, language: string, source: string): string {
  return `${storage.CODEBENCH_DRAFT_PREFIX}${username}:${pathname}:${language}:${fnv1a(source)}`;
}

export function serialize(draft: CodebenchDraft): string {
  return JSON.stringify(draft);
}

/** Absent, unparseable, or the wrong shape all read as `null` — a draft is a convenience, and one
 *  that cannot be trusted is not worth surfacing over the fence the reader can see. */
export function parse(raw: string | null): CodebenchDraft | null {
  if (raw === null) return null;
  try {
    const draft = JSON.parse(raw) as Partial<CodebenchDraft>;
    if (typeof draft.code !== "string") return null;
    if (typeof draft.stdin !== "string") return null;
    if (typeof draft.savedAt !== "number") return null;
    return { code: draft.code, stdin: draft.stdin, savedAt: draft.savedAt };
  } catch {
    return null;
  }
}

/** The saved draft for this fence, if one exists and still parses. A corrupt entry is dropped on
 *  the way past rather than left to fail the same way on every open. */
export function load(key: string): CodebenchDraft | null {
  const raw = storage.get(key);
  const draft = parse(raw);
  if (raw !== null && draft === null) {
    storage.remove(key);
    log.debug("codebench: dropped an unreadable draft");
  }
  return draft;
}

/** Persist the buffer (debounced by the caller). A denied write is a silent no-op — the accessor
 *  swallows it — so a storage-denied profile degrades to the old reset-on-reopen behaviour rather
 *  than breaking the editor. */
export function save(key: string, code: string, stdin: string): void {
  storage.set(key, serialize({ code, stdin, savedAt: Date.now() }));
}

/** Drop the draft — on Reset, or whenever the buffer matches the fence again. */
export function clear(key: string): void {
  storage.remove(key);
}
