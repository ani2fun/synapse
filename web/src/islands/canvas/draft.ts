// The canvas's in-progress draft. A reader who half-fills the Think pane and reloads — or wanders
// to the Code tab and back tomorrow — keeps what they wrote, because the alternative is a planning
// surface that silently discards planning.
//
// The draft lives in THIS browser and never reaches the server. It is not an entry: an entry is a
// deliberate snapshot the reader chose to keep, and those go to Postgres with the account, the way
// submissions do. The draft is the scratch state on the way to one, which is also why it works
// signed-out — thinking should not need an account.
//
// Two things make the key. The USERNAME, so a shared browser never shows one account's plan to the
// next (anonymous drafts get their own namespace and are never adopted on sign-in — a plan typed
// by whoever used this browser last is not this account's plan). And the PROBLEM PATH, so the same
// browser holds a draft per problem.
//
// The pure half (`keyFor`, `serialize`, `parse`) is separated from the storage half so it can be
// unit-tested: the vitest suite runs in node, with no `localStorage` at all.

import * as log from "../../lib/log";
import * as storage from "../../lib/storage";
import type { CanvasBody } from "./model";
import { normalizeBody, toWire } from "./model";

/** The anonymous namespace. Deliberately a name no Keycloak handle can be, so an anonymous draft
 *  and a signed-in one never collide. */
const ANON = "@anon";

export function keyFor(username: string | null, path: string[]): string {
  return `${storage.CANVAS_DRAFT_PREFIX}${username ?? ANON}:${path.join("/")}`;
}

interface StoredDraft {
  /** The wire body shape, so a draft and an exported entry read the same. */
  body: ReturnType<typeof toWire>;
  /** epoch ms — what an expiry sweep would need, one field now versus a migration later. */
  savedAt: number;
}

export function serialize(body: CanvasBody): string {
  return JSON.stringify({ body: toWire(body), savedAt: Date.now() } satisfies StoredDraft);
}

/** Absent, unparseable, or the wrong shape all read as `null`. `normalizeBody` is tolerant of a
 *  body missing fields, so only an outright non-object is rejected here — a draft written by an
 *  older build that lacked an area should still come back, minus that area. */
export function parse(raw: string | null): CanvasBody | null {
  if (raw === null) return null;
  try {
    const stored = JSON.parse(raw) as Partial<StoredDraft>;
    if (typeof stored !== "object" || stored === null) return null;
    if (typeof stored.body !== "object" || stored.body === null) return null;
    return normalizeBody(stored.body);
  } catch {
    return null;
  }
}

/** The saved draft for this problem, if one exists and still parses. A corrupt entry is dropped on
 *  the way past rather than left to fail identically on every open. */
export function load(key: string): CanvasBody | null {
  const raw = storage.get(key);
  const draft = parse(raw);
  if (raw !== null && draft === null) {
    storage.remove(key);
    log.debug("canvas: dropped an unreadable draft");
  }
  return draft;
}

/** Persist the draft (debounced by the caller). A denied write is a silent no-op — the accessor
 *  swallows it — so a storage-denied profile degrades to losing the draft on reload rather than
 *  breaking the pane. */
export function save(key: string, body: CanvasBody): void {
  storage.set(key, serialize(body));
}

export function clear(key: string): void {
  storage.remove(key);
}
