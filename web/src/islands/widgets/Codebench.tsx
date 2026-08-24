/**
 * The popup codebench: ONE near-fullscreen modal with ONE Monaco created on first open and
 * reused forever after (value + tokenizer swap). Run + editable stdin + the runnable output
 * panel ride along; Esc closes like every other popup; editing gates on sign-in while Run stays
 * open to everyone. Authors write bare fences — no `run` attribute, no markdown changes.
 *
 * A signed-in reader's edits SURVIVE the close: every click mints a fresh request object, so the
 * modal cannot recognise a fence by identity and instead re-derives a per-account key from the
 * page and the authored source (`codebenchDraft.ts`) and reads its draft back. Editing is what
 * creates a draft and matching the fence again is what removes one, so "Reset to the original"
 * is the only way home — closing and reopening no longer reverts anything.
 *
 * The button that opens it lives in the fence group's header bar (`fenceGroups.ts`); this module
 * keeps the store, the modal, and (in `../../lib/execution/language.ts`) the alias table that
 * decides which fences get one.
 */
import { useEffect, useRef, useState } from "preact/hooks";

import { clear as clearDraft, keyFor, load as loadDraft, save as saveDraft } from "./codebenchDraft";
import { displayLang } from "../../lib/execution/blocks";
import * as executor from "../../lib/execution/executor";
import type { EditorHandle } from "../../lib/islands/editor/monaco";
import * as log from "../../lib/log";
import { Store, useStore } from "../../lib/store";
import { AUTH_CHANGED, currentUser, isAuthed } from "../workbench/contracts";
import { Output } from "../workbench/panels";
import { BlockStore } from "../workbench/state";

/** The draft is a convenience, not a document; it can lag the keystroke it belongs to. Matches
 *  the diagram lab's autosave. */
const DRAFT_DEBOUNCE_MS = 800;

// ─────────────────────────────────────────────────────────────────────────────
// THE STORE (the CodebenchStore singleton pattern)
// ─────────────────────────────────────────────────────────────────────────────

export interface CodebenchRequest {
  code: string;
  language: string;
}

export const codebenchStore = new Store<CodebenchRequest | null>(null);

export function openCodebench(request: CodebenchRequest): void {
  log.info(`codebench: opening a ${request.language} snippet`);
  codebenchStore.set(request);
}

function closeCodebench(): void {
  codebenchStore.set(null);
}

const PLAY = (cls: string) => (
  <svg class={cls} viewBox="0 0 24 24" width="12" height="12" fill="currentColor" aria-hidden="true">
    <path d="M8 5v14l11-7z"></path>
  </svg>
);

// ─────────────────────────────────────────────────────────────────────────────
// THE MODAL — one Monaco, reused forever
// ─────────────────────────────────────────────────────────────────────────────

/** Mounted once, page-wide (`widgets/index.ts`). The frame stays in the DOM across opens
 *  (hidden via `.codebench`/`.codebench--open` in codebench.css) so the single Monaco instance
 *  survives; each open swaps value + tokenizer in place. */
export function CodebenchModal() {
  const request = useStore(codebenchStore);
  const [authed, setAuthed] = useState(isAuthed());
  const [block] = useState(() => new BlockStore(""));
  const state = useStore(block.state);
  const [stdin, setStdin] = useState("");
  // Written only by `applyStdin`, never during render: a render-time assignment would undo a
  // just-applied value if anything else re-rendered before the state flushed.
  const stdinRef = useRef("");
  const requestRef = useRef(request);
  requestRef.current = request;
  const editorHost = useRef<HTMLDivElement>(null);
  const mounted = useRef<EditorHandle | null>(null);
  /** The key this open is saving under — null while anonymous (no account to key on) or closed,
   *  which is what makes the whole draft feature inert for a signed-out reader. */
  const draftKeyRef = useRef<string | null>(null);
  const saveTimer = useRef<number | null>(null);

  useEffect(() => {
    const onAuth = () => setAuthed(isAuthed());
    window.addEventListener(AUTH_CHANGED, onAuth);
    return () => window.removeEventListener(AUTH_CHANGED, onAuth);
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && codebenchStore.get() != null) closeCodebench();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const run = () => {
    const r = requestRef.current;
    if (!r) return;
    block.launch(r.language, stdinRef.current === "" ? null : stdinRef.current);
  };
  const runRef = useRef(run);
  runRef.current = run;

  /**
   * Set stdin through here, never through `setStdin` alone.
   *
   * `scheduleSave` reads the REF, and a ref assigned during render still holds the previous value
   * when a save is scheduled synchronously from the same handler. Both paths that replace the
   * buffer do exactly that — `setValue` fires Monaco's `onChange` on the spot — so Reset would
   * write the stdin it just cleared, and a restore would write the previous fence's stdin under
   * the new fence's key.
   */
  const applyStdin = (next: string) => {
    stdinRef.current = next;
    setStdin(next);
  };

  /** The key for whatever is open, or null when nobody is signed in to own it. */
  const keyForOpen = (r: CodebenchRequest): string | null => {
    const user = currentUser();
    return user === null ? null : keyFor(user, window.location.pathname, r.language, r.code);
  };

  /**
   * Persist the buffer, debounced.
   *
   * Driven from the change callbacks rather than an effect over the buffer: on the commit where
   * `request` flips to another fence, an effect would briefly pair the PREVIOUS fence's buffer
   * with the NEW fence's key. Reading both here keeps the code and the key it belongs to
   * inseparable.
   */
  const scheduleSave = () => {
    const key = draftKeyRef.current;
    const authored = requestRef.current?.code;
    if (key === null || authored === undefined) return;
    if (saveTimer.current !== null) clearTimeout(saveTimer.current);
    const code = block.state.get().code;
    const stdin = stdinRef.current;
    saveTimer.current = window.setTimeout(() => {
      // A draft exists only while the bench differs from the fence — so editing back to the
      // original leaves nothing behind, and Reset needs no separate bookkeeping.
      if (code === authored && stdin === "") clearDraft(key);
      else saveDraft(key, code, stdin);
    }, DRAFT_DEBOUNCE_MS);
  };
  const scheduleSaveRef = useRef(scheduleSave);
  scheduleSaveRef.current = scheduleSave;

  /** Back to the fence, and drop the draft with it. The codebench has no other way home: closing
   *  and reopening used to be the revert, and persistence is exactly what takes that away. */
  const resetToFence = () => {
    const r = requestRef.current;
    if (!r) return;
    if (saveTimer.current !== null) {
      clearTimeout(saveTimer.current);
      saveTimer.current = null;
    }
    if (draftKeyRef.current !== null) clearDraft(draftKeyRef.current);
    block.state.update((s) => executor.setCode(s, r.code));
    // Before `setValue`, whose `onChange` schedules a save that reads it.
    applyStdin("");
    mounted.current?.setValue(r.code);
    log.debug("codebench: reset to the authored fence");
  };

  // Each open restores the bench for THIS fence — a saved draft when one applies, the authored
  // source otherwise — and resets the FSM around it; the editor (if already alive) swaps value +
  // tokenizer in place. Every click mints a fresh request object, so this runs on re-opening the
  // same fence too: the draft, not object identity, is what carries the buffer across.
  useEffect(() => {
    if (!request) return;
    draftKeyRef.current = keyForOpen(request);
    const draft = draftKeyRef.current === null ? null : loadDraft(draftKeyRef.current);
    const opening = draft?.code ?? request.code;
    if (draft) log.debug(`codebench: restored an edited buffer (${request.language})`);
    block.state.set(executor.initial(opening));
    applyStdin(draft?.stdin ?? "");
    if (mounted.current) {
      mounted.current.setValue(opening);
      mounted.current.setLanguage(request.language);
      mounted.current.setReadOnly(!isAuthed());
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request]);

  // First open mounts the ONE editor; it lives for the rest of the session.
  useEffect(() => {
    if (!request || mounted.current) return;
    const node = editorHost.current;
    if (!node) return;
    void (async () => {
      const { createEditor } = await import("../../lib/islands/editor/monaco");
      if (mounted.current) return;
      const dark = document.documentElement.classList.contains("dark");
      const handle = createEditor(node, {
        // The restore effect above is declared first and has already run, so the store holds
        // whatever this open should show — the draft, not necessarily the fence.
        value: block.state.get().code,
        language: request.language,
        readOnly: !isAuthed(),
        dark,
        onChange: (code: string) => {
          block.state.update((s) => executor.setCode(s, code));
          scheduleSaveRef.current();
        },
        onRun: () => runRef.current(),
        onToggleEdit: () => {},
      });
      log.debug(`codebench monaco mounted (${request.language})`);
      mounted.current = handle;
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request]);

  // Signing in mid-session unlocks the buffer in place — and gives the draft an owner to key on,
  // without which the newly writable editor would save nothing. RECOMPUTE only, never re-load:
  // pulling a stored draft into a buffer someone is already typing in would destroy the very
  // edits this exists to keep.
  useEffect(() => {
    mounted.current?.setReadOnly(!authed);
    const r = requestRef.current;
    draftKeyRef.current = r ? keyForOpen(r) : null;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [authed]);

  // A pending write outlives a close (the modal stays mounted, so the timer still fires) but must
  // not outlive the component.
  useEffect(
    () => () => {
      if (saveTimer.current !== null) clearTimeout(saveTimer.current);
    },
    [],
  );

  // The theme follows the toggle (the same `<html>.dark` observer every editor host uses).
  useEffect(() => {
    const observer = new MutationObserver(() =>
      mounted.current?.setTheme(document.documentElement.classList.contains("dark")),
    );
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, []);

  const running = state.runState === "running";
  const dirty = request != null && executor.isDirty(state, request.code);
  const changed = request == null ? 0 : executor.changedLineCount(state, request.code);

  return (
    <div class={request ? "codebench codebench--open" : "codebench"}>
      <div class="codebench__scrim" onClick={closeCodebench}></div>
      <div class="codebench__frame">
        <div class="codebench__bar">
          <span class="wb__eyebrow">
            <span class="wb__prompt">⤢</span> CODEBENCH
          </span>
          <span class="wb__lang-pill">
            {PLAY("wb__lang-play")}
            <span>{request ? displayLang(request.language) : ""}</span>
          </span>
          <span class="codebench__spacer"></span>
          <button class="runnable__run" disabled={running} title="Run (⌘⏎)" onClick={() => runRef.current()}>
            {PLAY("runnable__run-ic")}
            <span>{running ? "Running…" : "Run"}</span>
          </button>
          <button class="codebench__close" aria-label="Close (Esc)" onClick={closeCodebench}>
            ✕
          </button>
        </div>
        {!authed && (
          <div class="wb__edit-bar codebench__signin">
            <span class="wb__edit-status">
              <span class="wb__edit-dot"></span>
              Sign in to edit — you can still Run it as written
            </span>
          </div>
        )}
        {/* Gated on the buffer differing, not on being signed in: after a sign-out this bar is the
            only thing explaining why the code on screen is not the fence. */}
        {dirty && (
          <div class="wb__edit-bar codebench__draft">
            <span class="wb__edit-status">
              <span class="wb__edit-dot"></span>
              Your edits are saved in this browser — {changed} line{changed === 1 ? "" : "s"} changed
            </span>
            <button class="wb__ghost" onClick={resetToFence}>
              Reset to the original
            </button>
          </div>
        )}
        <div class="codebench__editor" ref={editorHost}></div>
        <div class="codebench__stdin">
          <label class="viz-stdin__label">stdin</label>
          <textarea
            class="viz-stdin__input"
            rows={2}
            placeholder="Input the program reads, one line per read"
            value={stdin}
            onInput={(event) => {
              applyStdin((event.target as HTMLTextAreaElement).value);
              scheduleSaveRef.current();
            }}
          ></textarea>
        </div>
        <div class="codebench__out">
          <Output state={state} tests={null} />
        </div>
      </div>
    </div>
  );
}
