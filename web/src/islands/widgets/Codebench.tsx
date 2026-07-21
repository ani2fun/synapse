/**
 * The popup codebench (port of client/src/execution/view/codebench.rs — qna Q1, option A): ONE
 * near-fullscreen modal with ONE Monaco created on first open and reused forever after (value +
 * tokenizer swap, the step-30 seam). Run + editable stdin + the runnable output panel ride
 * along; Esc closes like every other popup; editing gates on sign-in while Run stays open to
 * everyone. Authors write bare fences — no `run` attribute, no markdown changes.
 *
 * The button that opens it lives in the fence group's header bar (`fenceGroups.ts`, step 41);
 * this module keeps the store, the modal, and (in `../../lib/execution/language.ts`) the alias
 * table that decides which fences get one.
 */
import { useEffect, useRef, useState } from "preact/hooks";

import { displayLang } from "../../lib/execution/blocks";
import * as executor from "../../lib/execution/executor";
import type { EditorHandle } from "../../lib/islands/editor/monaco";
import * as log from "../../lib/log";
import { Store, useStore } from "../../lib/store";
import { AUTH_CHANGED, isAuthed } from "../workbench/contracts";
import { Output } from "../workbench/panels";
import { BlockStore } from "../workbench/state";

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
  const stdinRef = useRef("");
  stdinRef.current = stdin;
  const requestRef = useRef(request);
  requestRef.current = request;
  const editorHost = useRef<HTMLDivElement>(null);
  const mounted = useRef<EditorHandle | null>(null);

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

  // Each open resets the bench to the fence: FSM + buffer + stdin; the editor (if already alive)
  // swaps value + tokenizer in place.
  useEffect(() => {
    if (!request) return;
    block.state.set(executor.initial(request.code));
    setStdin("");
    if (mounted.current) {
      mounted.current.setValue(request.code);
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
        value: request.code,
        language: request.language,
        readOnly: !isAuthed(),
        dark,
        onChange: (code: string) => block.state.update((s) => executor.setCode(s, code)),
        onRun: () => runRef.current(),
        onToggleEdit: () => {},
      });
      log.debug(`codebench monaco mounted (${request.language})`);
      mounted.current = handle;
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request]);

  // Signing in mid-session unlocks the buffer in place.
  useEffect(() => {
    mounted.current?.setReadOnly(!authed);
  }, [authed]);

  // The theme follows the toggle (the same `<html>.dark` observer every editor host uses).
  useEffect(() => {
    const observer = new MutationObserver(() =>
      mounted.current?.setTheme(document.documentElement.classList.contains("dark")),
    );
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, []);

  const running = state.runState === "running";

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
        <div class="codebench__editor" ref={editorHost}></div>
        <div class="codebench__stdin">
          <label class="viz-stdin__label">stdin</label>
          <textarea
            class="viz-stdin__input"
            rows={2}
            placeholder="Input the program reads, one line per read"
            value={stdin}
            onInput={(event) => setStdin((event.target as HTMLTextAreaElement).value)}
          ></textarea>
        </div>
        <div class="codebench__out">
          <Output state={state} tests={null} />
        </div>
      </div>
    </div>
  );
}
