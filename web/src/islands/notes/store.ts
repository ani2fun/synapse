// A problem's notes: a copy in THIS browser, and — for a signed-in reader — the account's copy on
// the server (`/api/notes`), which is what follows them to another machine.
//
// The local copy is not just an anonymous fallback. It is written first on every edit, so a note
// survives a dropped connection or a closed tab, and it carries two facts sync needs:
//   · `synced` — whether the server has this exact text. An unsynced copy is pushed on the next
//     open, which is what makes a save lost to `pagehide` merely late rather than gone.
//   · `base` — the server's `updatedAt` this copy started from. If the server still says `base`,
//     nobody else has written since, and local edits can go up without a conflict.
//
// The key is the canvas draft's shape: USERNAME, so a shared browser never shows one account's
// notes to the next (anonymous notes live under their own name and are never adopted on sign-in),
// and PROBLEM PATH, so every problem keeps its own page.
//
// The pure half (`keyFor`, `serialize`, `parse`, `reconcile`) is separate from the storage half so
// the vitest suite, which runs in node with no `localStorage`, can pin it.

import * as storage from "../../lib/storage";
import { NOTES_MAX } from "./format";

/** No Keycloak handle can start with `@`, so anonymous and signed-in notes never collide. */
const ANON = "@anon";

export function keyFor(username: string | null, path: string[]): string {
  return `${storage.PROBLEM_NOTES_PREFIX}${username ?? ANON}:${path.join("/")}`;
}

export interface LocalNote {
  text: string;
  /** epoch ms of the last local write. */
  savedAt: number;
  /** The server holds exactly `text`. */
  synced: boolean;
  /** The server `updatedAt` this copy is based on; `null` when it never came from the server. */
  base: string | null;
}

export function serialize(note: LocalNote): string {
  return JSON.stringify(note);
}

/** Absent, unparseable or the wrong shape all read as `null`. Missing sync fields read as "never
 *  synced", so a note written before sync existed is pushed up the first time its owner opens it.
 *  An over-long note comes back truncated rather than refused — losing the tail is bad, losing all
 *  of it is worse. */
export function parse(raw: string | null): LocalNote | null {
  if (raw === null) return null;
  try {
    const stored = JSON.parse(raw) as Partial<LocalNote>;
    if (typeof stored !== "object" || stored === null || typeof stored.text !== "string") return null;
    return {
      text: stored.text.slice(0, NOTES_MAX),
      savedAt: typeof stored.savedAt === "number" ? stored.savedAt : 0,
      synced: stored.synced === true,
      base: typeof stored.base === "string" ? stored.base : null,
    };
  } catch {
    return null;
  }
}

/** The server's side of a reconcile: its text and when it was last written (`null` = no note). */
export interface ServerNote {
  text: string;
  updatedAt: string | null;
}

export interface Reconciled {
  /** What the pane should show. */
  text: string;
  /** Whether that text still has to be sent to the server. */
  push: boolean;
  /** The server version the shown text is based on. */
  base: string | null;
}

/**
 * Decide what to show when a signed-in reader opens a note: this browser's copy, the account's,
 * or neither yet. Runs once per open, after `GET /api/notes` answers.
 *
 * Nothing is merged. Notes are a scratchpad, so line-level merging would cost more than it saves.
 * On a real conflict the more recently written copy wins.
 */
export function reconcile(local: LocalNote | null, server: ServerNote): Reconciled {
  const adopt: Reconciled = { text: server.text, push: false, base: server.updatedAt };
  // Nothing local, or a local copy the server already has: the account's copy is the truth — even
  // an EMPTY one, which means the note was cleared from another machine.
  if (local === null || local.synced) return adopt;
  // Unsynced local edits that already match the server: nothing to send.
  if (local.text === server.text) return adopt;
  // The server has not moved since this copy was based on it, so the local edits are simply the
  // next version. This is the common case — a save that `pagehide` cut off.
  if (server.updatedAt === local.base) return { text: local.text, push: true, base: local.base };
  // A true conflict: both sides changed. The newer write wins, compared on wall clocks (this
  // browser's against the server's) — good enough for one person moving between their own
  // machines, which is the only way two copies of one account's note diverge.
  const serverAt = server.updatedAt === null ? 0 : Date.parse(server.updatedAt);
  return local.savedAt > serverAt ? { text: local.text, push: true, base: server.updatedAt } : adopt;
}

export function load(key: string): LocalNote | null {
  return parse(storage.get(key));
}

/** An empty, synced note removes its key: a problem you opened and never wrote in leaves nothing
 *  behind. An empty UNSYNCED one is kept — it is a deletion the server has not heard about yet. */
export function save(key: string, note: LocalNote): void {
  if (note.text === "" && note.synced) storage.remove(key);
  else storage.set(key, serialize(note));
}
