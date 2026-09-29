/**
 * The Notes tab — a markdown scratchpad beside the code, one page per problem.
 *
 * WRITE is a plain textarea with a toolbar that types markdown for you (`format.ts`); PREVIEW
 * renders it through the notes pipeline (`render.ts`), which drops raw HTML and unsafe URLs
 * because a note is untrusted text going into `innerHTML`. The two are one document viewed two
 * ways, so the toolbar is disabled in Preview rather than hidden — the bar keeps its place and
 * the reader can see why nothing responds.
 *
 * The note autosaves, so there is no Save button to forget. Every edit lands in this browser first
 * (`store.ts`, debounced and flushed on `pagehide`/`visibilitychange` like the canvas draft); for a
 * signed-in reader it is then pushed to the account (`PUT /api/notes`) on a longer debounce, and
 * each open reconciles the two copies (`store.reconcile`). Signed-out, notes stay on this device.
 *
 * The textarea is UNCONTROLLED: the element's value is the buffer. The toolbar writes through
 * `execCommand("insertText")` over the smallest changed range (`spliceOf`), which is what keeps
 * each toolbar press on the browser's own undo stack — assigning `.value` would wipe Cmd+Z.
 */
import type { JSX } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import * as log from "../../lib/log";
import * as api from "../../lib/api/client";
import { AUTH_CHANGED, currentUser, isAuthed } from "../workbench/contracts";
import { applyFormat, NOTES_MAX, spliceOf } from "./format";
import type { FormatAction } from "./format";
import * as store from "./store";

const AUTOSAVE_MS = 400;
/** Slower than the local write: the server hears about a pause in typing, not every word. */
const PUSH_MS = 1500;
const TOAST_MS = 2200;

type SyncStatus = "idle" | "pending" | "local" | "syncing" | "synced" | "failed";

function statusLabel(status: SyncStatus, authed: boolean): string {
  switch (status) {
    case "pending":
      return "Saving…";
    case "syncing":
      return "Syncing…";
    case "synced":
      return "Synced to your account";
    case "failed":
      return "Sync failed — kept on this device";
    case "local":
      return authed ? "Saved on this device" : "Saved on this device · sign in to sync";
    case "idle":
      return authed ? "Syncs to your account" : "Autosaves on this device";
  }
}

const IS_MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
const MOD = IS_MAC ? "⌘" : "Ctrl+";

const icon = (paths: string): JSX.Element => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d={paths} />
  </svg>
);

interface Tool {
  action: FormatAction;
  label: string;
  /** The key that fires it with the platform modifier, if any. */
  key?: string;
  glyph: JSX.Element;
}

/** Grouped the way the reader reaches for them: inline emphasis, block structure, references,
 *  then the two block-level containers. Each group is separated by a rule in the bar. */
const TOOLS: Tool[][] = [
  [
    { action: "bold", label: "Bold", key: "b", glyph: icon("M6 12h9a4 4 0 0 1 0 8H7a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1h7a4 4 0 0 1 0 8") },
    { action: "italic", label: "Italic", key: "i", glyph: icon("M19 4h-9 M14 20H5 M15 4L9 20") },
    { action: "strike", label: "Strikethrough", glyph: icon("M16 4H9a3 3 0 0 0-2.83 4 M14 12a4 4 0 0 1 0 8H6 M4 12h16") },
  ],
  [
    { action: "bullet", label: "Bulleted list", glyph: icon("M3 6h.01 M3 12h.01 M3 18h.01 M8 6h13 M8 12h13 M8 18h13") },
    { action: "numbered", label: "Numbered list", glyph: icon("M10 6h11 M10 12h11 M10 18h11 M4 6h1v4 M4 10h2 M6 18H4c0-1 2-2 2-3s-1-1.5-2-1") },
    { action: "task", label: "Checklist", glyph: icon("M3 7l2 2 4-4 M3 17l2 2 4-4 M13 6h8 M13 12h8 M13 18h8") },
  ],
  [
    { action: "heading", label: "Heading", glyph: icon("M4 12h8 M4 18V6 M12 18V6 M21 18h-4c0-4 4-3 4-6 0-1.5-2-2.5-4-1") },
    { action: "code", label: "Inline code", key: "e", glyph: icon("M16 18l6-6-6-6 M8 6l-6 6 6 6") },
  ],
  [
    { action: "link", label: "Link", key: "k", glyph: icon("M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71 M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71") },
    { action: "image", label: "Image", glyph: icon("M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z M9 11a2 2 0 1 0 0-4 2 2 0 0 0 0 4z M21 15l-3.09-3.09a2 2 0 0 0-2.82 0L6 21") },
  ],
  [
    { action: "codeblock", label: "Code block", glyph: icon("M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z M10 9l-3 3 3 3 M14 15l3-3-3-3") },
    { action: "quote", label: "Quote", glyph: icon("M3 21c3 0 7-1 7-8V5c0-1.25-.76-2-2-2H4c-1.25 0-2 .75-2 1.97V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .01-1 1.03V20c0 1 0 1 1 1z M15 21c3 0 7-1 7-8V5c0-1.25-.76-2-2-2h-4c-1.25 0-2 .75-2 1.97V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z") },
  ],
];

/** Modifier+key → action, built once from the table so a shortcut and its tooltip cannot drift. */
const SHORTCUTS = new Map(TOOLS.flat().filter((t) => t.key).map((t) => [t.key!, t.action]));

export function NotesPane({ path }: { path: string[] }) {
  const field = useRef<HTMLTextAreaElement>(null);
  const preview = useRef<HTMLDivElement>(null);
  const key = useRef(store.keyFor(currentUser(), path));
  const text = useRef("");
  /** What the field was LOADED with — stable between loads. Handing Preact the live buffer as
   *  `defaultValue` instead would rewrite the textarea's text node on every keystroke's render,
   *  and Chrome drops the undo history whenever that happens. */
  const loaded = useRef("");
  const saveTimer = useRef<number | undefined>(undefined);
  const pushTimer = useRef<number | undefined>(undefined);
  const toastTimer = useRef<number | undefined>(undefined);
  // What sync needs to know about the buffer (see `store.ts`): whether the server holds exactly
  // this text, which server version it grew from, and when it was last edited here.
  const synced = useRef(true);
  const base = useRef<string | null>(null);
  const editedAt = useRef(0);
  // One push in flight at a time; an edit during it queues exactly one more. `generation` bumps
  // when the owner changes, so a reply for the previous account is dropped rather than applied.
  const inFlight = useRef(false);
  const again = useRef(false);
  const generation = useRef(0);

  const [fieldKey, setFieldKey] = useState("n0");
  const [count, setCount] = useState(0);
  const [mode, setMode] = useState<"write" | "preview">("write");
  const [status, setStatus] = useState<SyncStatus>("idle");
  const [toast, setToast] = useState<string | null>(null);

  const say = (message: string) => {
    setToast(message);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), TOAST_MS);
  };

  /** Write the local copy NOW. Always first, and always synchronous: it is the copy that
   *  survives a closed tab, and the `synced` flag in it is what gets a lost push retried. */
  const writeLocal = () => {
    window.clearTimeout(saveTimer.current);
    saveTimer.current = undefined;
    store.save(key.current, {
      text: text.current,
      savedAt: editedAt.current,
      synced: synced.current,
      base: base.current,
    });
  };

  const flushLocal = () => {
    if (saveTimer.current !== undefined) writeLocal();
  };

  /** Send the buffer to the account. Signed-out, or already synced, it does nothing. */
  const push = async (): Promise<void> => {
    window.clearTimeout(pushTimer.current);
    pushTimer.current = undefined;
    if (!isAuthed() || synced.current) return;
    if (inFlight.current) {
      again.current = true;
      return;
    }
    inFlight.current = true;
    const sent = text.current;
    const owner = generation.current;
    setStatus("syncing");
    try {
      const note = await api.saveNote({ path, text: sent });
      if (owner !== generation.current) return;
      base.current = note.updatedAt ?? null;
      // Typing may have carried on while the request was out; only the text that was SENT is
      // known to be on the server.
      if (text.current === sent) {
        synced.current = true;
        setStatus("synced");
      }
      writeLocal();
      log.debug(`notes: pushed ${sent.length} char(s)`);
    } catch (error) {
      if (owner !== generation.current) return;
      setStatus("failed");
      log.warn(`notes: sync failed — kept on this device (${String(error)})`);
    } finally {
      inFlight.current = false;
      if (again.current) {
        again.current = false;
        void push();
      }
    }
  };

  const onText = (value: string) => {
    text.current = value;
    editedAt.current = Date.now();
    synced.current = false;
    setCount(value.length);
    setStatus("pending");
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      writeLocal();
      if (!isAuthed()) setStatus("local");
    }, AUTOSAVE_MS);
    if (isAuthed()) {
      window.clearTimeout(pushTimer.current);
      pushTimer.current = window.setTimeout(() => void push(), PUSH_MS);
    }
  };

  /** Put a new document into the field — remounting is the only way into an uncontrolled one. */
  const showText = (value: string) => {
    text.current = value;
    loaded.current = value;
    setCount(value.length);
    setFieldKey(`n${Date.now()}`);
  };

  /** Ask the account for its copy and settle the two (`store.reconcile`). */
  const syncOpen = () => {
    if (!isAuthed()) return;
    const owner = generation.current;
    const local = store.load(key.current);
    setStatus("syncing");
    api.noteFor(path).then(
      (server) => {
        if (owner !== generation.current) return;
        // The reader started typing before the answer came back. What they are typing is the
        // newest thing anywhere, so it goes up rather than being replaced under their cursor.
        if (text.current !== loaded.current) {
          base.current = server.updatedAt ?? null;
          void push();
          return;
        }
        const settled = store.reconcile(local, { text: server.text, updatedAt: server.updatedAt ?? null });
        base.current = settled.base;
        synced.current = !settled.push;
        if (settled.text !== text.current) {
          log.info(`notes: ${settled.push ? "kept this device's copy" : "loaded the account's copy"}`);
          showText(settled.text);
        }
        writeLocal();
        if (settled.push) void push();
        else setStatus(server.updatedAt ? "synced" : "idle");
      },
      (error: unknown) => {
        if (owner !== generation.current) return;
        setStatus("failed");
        log.warn(`notes: could not read the account's copy (${String(error)})`);
      },
    );
  };

  /** Load the note under the CURRENT key: this browser's copy at once, then the account's. */
  const loadNote = () => {
    const note = store.load(key.current);
    synced.current = note?.synced ?? true;
    base.current = note?.base ?? null;
    editedAt.current = note?.savedAt ?? 0;
    showText(note?.text ?? "");
    setStatus(note ? "local" : "idle");
    syncOpen();
  };

  useEffect(() => {
    log.info(`notes pane mounted — /${path.join("/")}`);
    loadNote();
    // A sign-in or sign-out changes whose notes these are. Whatever was typed so far belongs to
    // the key it was typed under, so it is written THERE before the pane switches over.
    const onAuth = () => {
      flushLocal();
      window.clearTimeout(pushTimer.current);
      generation.current += 1;
      key.current = store.keyFor(currentUser(), path);
      log.debug("notes: auth changed — reloading under the new owner");
      loadNote();
    };
    // Leaving: the local write is what must land; the push is a best effort, and an unsynced
    // copy is pushed the next time this note opens anyway.
    const onLeave = () => {
      flushLocal();
      void push();
    };
    const onHide = () => {
      if (document.visibilityState === "hidden") onLeave();
    };
    const onOnline = () => void push();
    window.addEventListener(AUTH_CHANGED, onAuth);
    window.addEventListener("pagehide", onLeave);
    window.addEventListener("online", onOnline);
    document.addEventListener("visibilitychange", onHide);
    return () => {
      flushLocal();
      window.removeEventListener(AUTH_CHANGED, onAuth);
      window.removeEventListener("pagehide", onLeave);
      window.removeEventListener("online", onOnline);
      document.removeEventListener("visibilitychange", onHide);
      window.clearTimeout(pushTimer.current);
      window.clearTimeout(toastTimer.current);
    };
  }, []);

  // Preview renders on entry, from the buffer as it stands. The renderer is imported here and
  // not at the top, so unified only loads for a reader who actually previews.
  useEffect(() => {
    if (mode !== "preview") return;
    let live = true;
    const source = text.current;
    if (source.trim() === "") return;
    void import("./render")
      .then(({ renderNotes }) => renderNotes(source))
      .then((html) => {
        if (live && preview.current) preview.current.innerHTML = html;
      })
      .catch((error: unknown) => {
        log.error(`notes preview failed: ${String(error)}`);
        if (live && preview.current) preview.current.textContent = source;
      });
    return () => {
      live = false;
    };
  }, [mode]);

  const format = (action: FormatAction) => {
    const el = field.current;
    if (!el || mode !== "write") return;
    const before = el.value;
    const edit = applyFormat(before, el.selectionStart, el.selectionEnd, action);
    if (edit.text.length > NOTES_MAX) {
      say(`Note is full — ${NOTES_MAX.toLocaleString()} characters`);
      return;
    }
    const { from, to, insert } = spliceOf(before, edit.text);
    el.focus();
    el.setSelectionRange(from, to);
    // `execCommand` is deprecated but remains the only way to edit a textarea that the browser
    // records for undo. Where it is gone or refuses (an empty insert, in some engines), the value
    // is set outright: the edit still lands, only its undo step is lost.
    const recorded = typeof document.execCommand === "function" && document.execCommand("insertText", false, insert);
    if (!recorded || el.value !== edit.text) el.value = edit.text;
    el.setSelectionRange(edit.start, edit.end);
    onText(el.value);
    log.debug(`notes: ${action}`);
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (!(IS_MAC ? event.metaKey : event.ctrlKey) || event.altKey || event.shiftKey) return;
    const action = SHORTCUTS.get(event.key.toLowerCase());
    if (!action) return;
    event.preventDefault();
    format(action);
  };


  return (
    <div class="pnotes">
      <div class="pnotes__bar">
        <div class="pnotes__tools" role="toolbar" aria-label="Formatting">
          {TOOLS.map((group, i) => (
            <span class="pnotes__group" key={i}>
              {group.map((tool) => {
                const hint = tool.key ? `${tool.label} (${MOD}${tool.key.toUpperCase()})` : tool.label;
                return (
                  <button
                    key={tool.action}
                    class="pnotes__tool"
                    type="button"
                    title={hint}
                    aria-label={hint}
                    disabled={mode !== "write"}
                    // Keep focus (and so the selection) in the textarea: a button that takes focus
                    // on mousedown collapses the very selection it is about to format.
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => format(tool.action)}
                  >
                    {tool.glyph}
                  </button>
                );
              })}
            </span>
          ))}
        </div>
        <span class="pnotes__spacer" />
        <span class="pcanvas__seg">
          <button
            class={`pcanvas__seg-btn${mode === "write" ? " pcanvas__seg-btn--on" : ""}`}
            type="button"
            aria-pressed={mode === "write"}
            onClick={() => setMode("write")}
          >
            Write
          </button>
          <button
            class={`pcanvas__seg-btn${mode === "preview" ? " pcanvas__seg-btn--on" : ""}`}
            type="button"
            aria-pressed={mode === "preview"}
            onClick={() => {
              flushLocal();
              setMode("preview");
            }}
          >
            Preview
          </button>
        </span>
      </div>

      <div class="pnotes__body">
        {/* Hidden, not unmounted, in Preview: the textarea IS the buffer, and keeping it mounted
            keeps its caret, scroll and undo history for the trip back to Write. */}
        <textarea
          key={fieldKey}
          ref={field}
          class="pnotes__field"
          hidden={mode !== "write"}
          aria-label="Notes"
          placeholder="Start writing"
          maxLength={NOTES_MAX}
          spellcheck={true}
          defaultValue={loaded.current}
          onInput={(event) => onText((event.currentTarget as HTMLTextAreaElement).value)}
          onKeyDown={onKeyDown}
        />
        {mode === "preview" &&
          (text.current.trim() === "" ? (
            <p class="pnotes__empty">Nothing to preview yet.</p>
          ) : (
            <div ref={preview} class="pnotes__preview synapse-prose" />
          ))}
      </div>

      <div class="pnotes__foot">
        <span class={`pnotes__count${count >= NOTES_MAX ? " pnotes__count--full" : ""}`}>
          {count.toLocaleString()} / {NOTES_MAX.toLocaleString()} characters
        </span>
        <span class="pnotes__spacer" />
        <span class={`pnotes__status pnotes__status--${status}`}>{statusLabel(status, isAuthed())}</span>
      </div>

      {toast && <div class="pcanvas__toast">{toast}</div>}
    </div>
  );
}
