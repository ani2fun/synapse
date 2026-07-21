/**
 * One runnable code block (port of client/src/execution/view/runnable.rs — the migration's
 * risk-1 component). Multi-variant language tabs over ONE Monaco; each variant keeps its own
 * buffer/run state in its own BlockStore; switching swaps the editor's value + tokenizer in
 * place. Viewport-lazy Monaco with the page-wide cap; shiki placeholder until then.
 *
 * Deliberately absent this step, per the migration plan: the auth island (A11) — until it
 * installs `window.__synapseAuth`, Edit and Submit render disabled with the sign-in copy,
 * which is exactly the anonymous experience; the viz island (A10) — Visualise renders only
 * once `window.__synapseViz` exists.
 */
import { useEffect, useMemo, useRef, useState } from "preact/hooks";

import { displayLang, canVisualise, expectedFor, seedValues } from "../../lib/execution/blocks";
import type { Variant } from "../../lib/execution/blocks";
import * as executor from "../../lib/execution/executor";
import { judge, stdinFor } from "../../lib/execution/judge";
import type { TestSpec } from "../../lib/execution/judge";
import { canonicalLang, preferredIndex } from "../../lib/execution/language";
import type { EditorHandle } from "../../lib/islands/editor/monaco";
import { WB_LANGUAGE_KEY, get as storageGet, set as storageSet } from "../../lib/storage";
import { Store, useStore } from "../../lib/store";
import {
  AUTH_CHANGED,
  CODE_CHANGED,
  LOAD_CODE,
  SUBMITTED,
  USE_CASE,
  isAuthed,
} from "./contracts";
import type { LoadCode, UseCase } from "./contracts";
import * as log from "../../lib/log";
import * as lazy from "./lazy";
import { Output, TestsPanel, VerdictPanel } from "./panels";
import { BlockStore, SubmitStore, TestsState } from "./state";

/** The oracle's editor height rule (client/src/islands/editor.rs). */
function defaultHeightPx(source: string): number {
  const lines = source.split("\n").length;
  return Math.min(Math.max(lines * 20 + 28, 64), 520);
}

const PLAY = (cls: string) => (
  <svg class={cls} viewBox="0 0 24 24" width="12" height="12" fill="currentColor" aria-hidden="true">
    <path d="M8 5v14l11-7z"></path>
  </svg>
);

export interface WorkbenchProps {
  variants: Variant[];
  spec: TestSpec | null;
  lessonPath: string[];
  /** The hydrated placeholder — the event-contract surface (load-code / use-case listeners,
   *  submitted / code-changed dispatches). */
  root: HTMLElement;
  /** Embedded practice (step 30): Run only — the Submit verb never renders. */
  practice?: boolean;
  /** Problem page right pane: editor fills the free height until a drag pins one. */
  fill?: boolean;
}

export function Workbench({ variants, spec, lessonPath, root, practice = false, fill = false }: WorkbenchProps) {
  // ── stores, minted once ──
  const stores = useMemo(() => variants.map((v) => new BlockStore(v.source)), []);
  const submit = useMemo(() => new SubmitStore(), []);
  const tests = useMemo(() => (spec ? new TestsState(spec) : null), []);
  const start = useMemo(() => preferredIndex(variants, storageGet(WB_LANGUAGE_KEY)), []);
  const [active, setActive] = useState(start);
  const [menuOpen, setMenuOpen] = useState(false);
  const [authTick, setAuthTick] = useState(0);
  const [mountedTick, setMountedTick] = useState(0);
  const [preview, setPreview] = useState<string | null>(null);
  const [pinnedHeight, setPinnedHeight] = useState<number | null>(null);

  const editorHost = useRef<HTMLDivElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const mounted = useRef<EditorHandle | null>(null);
  const near = useRef(false);
  const wantsEditor = useRef(false);
  const registryId = useRef<number | null>(null);
  const copied = useState(false);

  const activeStore = stores[active]!;
  const state = useStore(activeStore.state);
  const unlocked = useStore(activeStore.unlocked);
  const submitState = useStore(submit.state);
  const authed = isAuthed();
  const hasSubmit = spec != null && !practice;
  const variant = variants[active]!;

  const emitCode = (source: string, language: string) => {
    root.dispatchEvent(
      new CustomEvent(CODE_CHANGED, { bubbles: true, detail: { source, language } }),
    );
  };

  // ── run, judged against the LAUNCHED case ──
  const stdin = () =>
    spec && tests ? stdinFor(tests.spec.get().args, tests.values.get()) : null;
  const run = () => {
    wantsEditor.current = true;
    if (tests) tests.ranCase.set(tests.activeCase.get());
    activeStore.launch(variant.language, stdin());
  };
  // The verdict recorder (the Rust Effect): a store subscription that reacts to a NEW runId
  // reaching done, judges against ranCase, and badges the chip.
  useEffect(() => {
    const seen = { id: null as number | null };
    const unsubs = stores.map((store, storeIndex) =>
      store.state.subscribe(() => {
        if (storeIndex !== active) return;
        const s = store.state.get();
        if (s.runState !== "done" || seen.id === s.runId || !tests) return;
        seen.id = s.runId;
        const ranCase = tests.ranCase.get();
        if (s.result && ranCase != null) {
          const expected = expectedFor(tests.spec.get(), ranCase);
          const verdict = judge(s.result, expected);
          log.debug(`case ${ranCase + 1} judged: ${verdict}`);
          tests.recordVerdict(ranCase, verdict);
        }
      }),
    );
    return () => unsubs.forEach((u) => u());
  }, [active]);

  // ── submit + the submitted tick ──
  const doSubmit = () => {
    if (!hasSubmit || !isAuthed()) return;
    submit.submit(lessonPath, variant.language, activeStore.state.get().code);
  };
  useEffect(() => {
    let wasDone = false;
    const unsub = submit.state.subscribe(() => {
      const done = submit.state.get().kind === "done";
      if (done && !wasDone) root.dispatchEvent(new CustomEvent(SUBMITTED, { bubbles: true }));
      wasDone = done;
    });
    return unsub;
  }, []);

  // ── tab switch: swap the ONE Monaco in place ──
  const switchTo = (index: number) => {
    if (index === active) return;
    log.debug(`workbench tab → ${variants[index]!.language}`);
    wantsEditor.current = true;
    setActive(index);
    const store = stores[index]!;
    const v = variants[index]!;
    const code = store.state.get().code;
    if (mounted.current) {
      mounted.current.setValue(code);
      mounted.current.setLanguage(v.language);
      mounted.current.setReadOnly(!store.unlocked.get());
    }
    emitCode(code, v.language);
  };

  // ── the event contracts on the hydrated root ──
  useEffect(() => {
    const onLoadCode = (event: Event) => {
      const { language, code } = (event as CustomEvent<LoadCode>).detail;
      // Canonical, not raw: a `python3` solution must find the `py` tab. Guarded on non-null
      // so two UNKNOWN languages don't both read as null and match.
      const wanted = canonicalLang(language);
      const target = variants.findIndex(
        (v) => wanted !== null && canonicalLang(v.language) === wanted,
      );
      const index = target >= 0 ? target : active;
      log.debug(`solution copied into the ${variants[index]!.language} tab`);
      switchTo(index);
      stores[index]!.state.update((s) => executor.setCode(s, code));
      mounted.current?.setValue(code);
      emitCode(code, variants[index]!.language);
    };
    const onUseCase = (event: Event) => {
      if (!tests) return;
      const { args, expected } = (event as CustomEvent<UseCase>).detail;
      log.debug("failing input pushed into the tests panel");
      const index = tests.append({ args, expected });
      onCaseSwitch(index);
    };
    const onAuth = () => setAuthTick((n) => n + 1);
    root.addEventListener(LOAD_CODE, onLoadCode);
    root.addEventListener(USE_CASE, onUseCase);
    window.addEventListener(AUTH_CHANGED, onAuth);
    return () => {
      root.removeEventListener(LOAD_CODE, onLoadCode);
      root.removeEventListener(USE_CASE, onUseCase);
      window.removeEventListener(AUTH_CHANGED, onAuth);
    };
  }, [active]);

  // ── chip switch clears every variant's stale run output; badges stay ──
  const onCaseSwitch = (_index: number) => {
    for (const store of stores) store.state.update((s) => executor.clearOutcome(s));
  };

  // ── viewport-lazy Monaco ──
  useEffect(() => {
    void import("../../lib/markdown/render").then(({ highlightCode }) =>
      highlightCode(variants[start]!.source, variants[start]!.language).then(setPreview),
    );
    const node = rootRef.current;
    if (!node) return;
    const watch = lazy.watchNear(node, (isNear) => {
      near.current = isNear;
      if (isNear && !mounted.current) {
        wantsEditor.current = true;
        setMountedTick((n) => n + 1);
      }
    });
    return () => {
      watch?.disconnect();
      if (registryId.current != null) lazy.deregister(registryId.current);
      mounted.current?.dispose();
      mounted.current = null;
      submit.dispose();
    };
  }, []);
  useEffect(() => {
    const node = editorHost.current;
    if (!node || mounted.current || !wantsEditor.current) return;
    const store = stores[active]!;
    const v = variants[active]!;
    void (async () => {
      // createEditor directly, not the loader shim: mountEditor's flat-positional signature is
      // the wasm-bindgen FFI shape for the OLD client. The dynamic import keeps Monaco lazy.
      const { createEditor } = await import("../../lib/islands/editor/monaco");
      if (mounted.current) return;
      const dark = document.documentElement.classList.contains("dark");
      const handle = createEditor(node, {
        value: store.state.get().code,
        language: v.language,
        readOnly: !store.unlocked.get(),
        dark,
        onChange: (code: string) => {
          const i = stores.indexOf(activeStoreRef.current);
          stores[i]!.state.update((s) => executor.setCode(s, code));
          emitCode(code, variants[i]!.language);
        },
        onRun: () => runRef.current(),
        onToggleEdit: () => toggleEditRef.current(),
        onSubmit: hasSubmit ? () => submitRef.current() : undefined,
      });
      log.debug(`monaco mounted (${v.language})`);
      mounted.current = handle;
      registryId.current = lazy.register(
        () => near.current,
        () => {
          // Eviction: drop the editor, refresh the placeholder from the LIVE buffer, re-arm.
          log.debug("monaco evicted (over the page cap, far from viewport)");
          mounted.current?.dispose();
          mounted.current = null;
          wantsEditor.current = false;
          registryId.current = null;
          const i = stores.indexOf(activeStoreRef.current);
          void import("../../lib/markdown/render").then(({ highlightCode }) =>
            highlightCode(stores[i]!.state.get().code, variants[i]!.language).then(setPreview),
          );
          setMountedTick((n) => n + 1);
        },
      );
      setMountedTick((n) => n + 1);
    })();
  }, [mountedTick, active]);

  // Latest-closure refs for the editor callbacks (mounted once, must see current state).
  const activeStoreRef = useRef(activeStore);
  activeStoreRef.current = activeStore;
  const runRef = useRef(run);
  runRef.current = run;
  const submitRef = useRef(doSubmit);
  submitRef.current = doSubmit;
  const toggleEdit = () => {
    if (!isAuthed()) return;
    wantsEditor.current = true;
    activeStore.toggleEdit(variant.source);
    if (mounted.current) {
      mounted.current.setReadOnly(!activeStore.unlocked.get());
      const code = activeStore.state.get().code;
      if (mounted.current.getValue() !== code) mounted.current.setValue(code);
    }
    setMountedTick((n) => n + 1);
  };
  const toggleEditRef = useRef(toggleEdit);
  toggleEditRef.current = toggleEdit;

  // Theme follows the toggle: the theme island flips .dark on <html>; observe it.
  useEffect(() => {
    const observer = new MutationObserver(() =>
      mounted.current?.setTheme(document.documentElement.classList.contains("dark")),
    );
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, []);

  // ── initial code snapshot for the coach seam ──
  useEffect(() => emitCode(variants[start]!.source, variants[start]!.language), []);

  // ── the resize strip ──
  const dragFrom = useRef<[number, number] | null>(null);
  useEffect(() => {
    const move = (event: PointerEvent) => {
      const from = dragFrom.current;
      if (!from) return;
      setPinnedHeight(Math.min(Math.max(from[1] + (event.clientY - from[0]), 140), 900));
    };
    const up = () => (dragFrom.current = null);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  }, []);

  const running = state.runState === "running";
  const judging = submitState.kind === "judging";
  const height = pinnedHeight ?? defaultHeightPx(variants[start]!.source);
  const vizReady = typeof window.__synapseViz === "function" && canVisualise(variant);

  const langChrome =
    variants.length > 1 ? (
      <div class="wb__lang">
        <button
          class="wb__lang-pill wb__lang-pill--btn"
          aria-label="Language"
          onClick={() => setMenuOpen((o) => !o)}
        >
          {PLAY("wb__lang-play")}
          <span>{displayLang(variant.language)}</span>
          <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <path d="m6 9 6 6 6-6"></path>
          </svg>
        </button>
        {menuOpen && (
          <div>
            <div class="wb__lang-scrim" onClick={() => setMenuOpen(false)}></div>
            <div class="wb__lang-menu">
              {variants.map((v, i) => (
                <button
                  class={`wb__lang-opt${active === i ? " wb__lang-opt--active" : ""}`}
                  onClick={() => {
                    storageSet(WB_LANGUAGE_KEY, v.language);
                    switchTo(i);
                    setMenuOpen(false);
                  }}
                >
                  {PLAY("wb__lang-play")}
                  {displayLang(v.language)}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    ) : (
      <span class="wb__lang-pill">
        {PLAY("wb__lang-play")}
        <span>{displayLang(variant.language)}</span>
      </span>
    );

  return (
    <div class="runnable not-prose" ref={rootRef} data-auth-tick={authTick}>
      <div class="runnable__bar">
        <span class="wb__eyebrow">
          <span class="wb__prompt">{">_"}</span> CODE
        </span>
        <span class="wb__actions">
          {langChrome}
          <span
            class="wb__tip"
            data-tip={
              !authed
                ? "Sign in to edit this code"
                : unlocked
                  ? "Editing — your changes stay on this page (⌘E toggles)"
                  : "Edit this code — changes stay on this page (⌘E)"
            }
          >
            <button class={`wb__ghost${unlocked ? " wb__ghost--live" : ""}`} disabled={!authed} onClick={toggleEdit}>
              {unlocked ? (
                <span>Editing</span>
              ) : (
                <>
                  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                    <rect width="18" height="11" x="3" y="11" rx="2" ry="2"></rect>
                    <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                  </svg>
                  <span>Edit</span>
                </>
              )}
            </button>
          </span>
          {unlocked && authed && (
            <button
              class="wb__ghost wb__ghost--live wb__ghost--icon"
              title="Restore the starter code"
              aria-label="Reset"
              onClick={() => {
                activeStore.state.update((s) => executor.setCode(s, variant.source));
                mounted.current?.setValue(variant.source);
              }}
            >
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"></path>
                <path d="M3 3v5h5"></path>
              </svg>
            </button>
          )}
          {hasSubmit && (
            <span
              class="wb__tip"
              data-tip={
                authed
                  ? "Submit against the hidden suite (⇧⌘⏎)"
                  : "Sign in to submit. Submit runs your code against every hidden test and saves your attempt. Saving needs an approved account — ask the operator for access."
              }
            >
              <button class="wb__submit" disabled={!authed || judging} onClick={doSubmit}>
                <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                  <path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z"></path>
                  <path d="m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z"></path>
                </svg>
                <span>{judging ? "Judging…" : "Submit"}</span>
              </button>
            </span>
          )}
          {vizReady && (
            <button
              class="wb__ghost"
              title="Trace this code and watch the structure animate"
              onClick={() =>
                window.__synapseViz?.({
                  language: variant.language,
                  source: activeStore.state.get().code,
                  vizHint: variant.viz ?? "",
                  stdin: stdin() ?? "",
                })
              }
            >
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"></path>
                <circle cx="12" cy="12" r="3"></circle>
              </svg>
              <span>Visualise</span>
            </button>
          )}
          <button class="runnable__run" title="Run (⌘⏎)" disabled={running} onClick={run}>
            {PLAY("runnable__run-ic")}
            <span>{running ? "Running…" : "Run"}</span>
          </button>
        </span>
      </div>
      <div
        class={fill && pinnedHeight == null ? "runnable__editor runnable__editor--fill" : "runnable__editor"}
        ref={editorHost}
        style={`height: ${height}px;`}
      >
        {!mounted.current && (
          <div
            class="runnable__preview"
            onClick={() => {
              wantsEditor.current = true;
              setMountedTick((n) => n + 1);
            }}
            dangerouslySetInnerHTML={{ __html: preview ?? "" }}
          ></div>
        )}
        <button
          class={`editor-copy${copied[0] ? " editor-copy--done" : ""}`}
          aria-label="Copy code"
          title="Copy code"
          onClick={() => {
            const code = mounted.current?.getValue() ?? activeStore.state.get().code;
            void navigator.clipboard?.writeText(code);
            copied[1](true);
            setTimeout(() => copied[1](false), 1400);
          }}
        >
          {copied[0] ? (
            <svg class="editor-copy__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
              <path d="M20 6 9 17l-5-5"></path>
            </svg>
          ) : (
            <svg class="editor-copy__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
              <rect x="8" y="8" width="14" height="14" rx="2" ry="2"></rect>
              <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path>
            </svg>
          )}
        </button>
      </div>
      {spec != null && (
        <div
          class="wb-hsplit"
          title="Drag to resize the editor — double-click to reset"
          onPointerDown={(event) => {
            event.preventDefault();
            const live = editorHost.current?.getBoundingClientRect().height ?? height;
            dragFrom.current = [event.clientY, live];
          }}
          onDblClick={() => setPinnedHeight(null)}
        >
          <div class="wb-hsplit__grip">
            <span></span>
            <span></span>
            <span></span>
          </div>
        </div>
      )}
      {tests && <TestsPanel tests={tests} onSwitch={onCaseSwitch} />}
      <Output state={state} tests={tests} />
      <VerdictPanel submit={submit} tests={tests} onSwitch={onCaseSwitch} />
    </div>
  );
}
