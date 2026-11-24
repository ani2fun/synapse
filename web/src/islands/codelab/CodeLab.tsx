/**
 * `/code` — the code lab. Write a program in any language the sandbox runs, give it input, read
 * what it printed. The editor is on the left; Input above Output on the right.
 *
 * It is a page of its own rather than the reader's workbench: the workbench carries a lesson's
 * chrome (tests, verdicts, the Edit lock), and the lab wants a different set — tabs, a language
 * menu, editor settings, files. Underneath it is the same layer the workbench uses: the Monaco
 * wrapper, `POST /api/run`, and the out-of-input reading in `lib/execution/judge`.
 *
 * ONE Monaco serves every tab. Switching swaps the buffer and the language in place, which keeps
 * a single lazy chunk and a single editor in memory; the cost is that undo history does not
 * survive a switch.
 *
 * THE BUFFER IS A REF. `tabsRef` holds the live tabs, sources included, and typing writes only
 * there — the `tabs` STATE re-renders the strip when a tab is added, renamed or recoloured, never
 * per keystroke. Anything that needs the code reads the ref.
 *
 * Everything stays in the browser: tabs, input and settings autosave to localStorage, and a share
 * link carries the lab in its URL fragment, which the server never sees.
 */
import { h, render } from "preact";
import { useCallback, useEffect, useRef, useState } from "preact/hooks";

import { run as runCode } from "../../lib/api/client";
import * as log from "../../lib/log";
import { mountCodeLabEditor } from "../../lib/islands/editor/loader";
import type { EditorHandle } from "../../lib/islands/editor/monaco";
import {
  CODE_LAB_ACTIVE_KEY,
  CODE_LAB_IO_PANE_KEY,
  CODE_LAB_PANE_KEY,
  CODE_LAB_SETTINGS_KEY,
  CODE_LAB_STDIN_KEY,
  CODE_LAB_TABS_KEY,
  get as storageGet,
  set as storageSet,
} from "../../lib/storage";
import { isDark } from "../../lib/theme";
import { LabGate } from "../labshell/LabGate";
import { LabSplit, useLabSplitter } from "../labshell/splitter";
import { THEME_CHANGED } from "../workbench/contracts";
import { downloadText, fileNameFor, pickTextFile } from "./files";
import { CLEAR, COPY, CROSS, DOWNLOAD, EXPAND, FORMAT, PLAY, PLUS, RESET, SHRINK, TICK, UPLOAD } from "./icons";
import { LanguageMenu } from "./LanguageMenu";
import { Output, outputText, type RunState } from "./Output";
import { parseSettings, type LabSettings } from "./settings";
import { SettingsPopover } from "./SettingsPopover";
import { decodeShare, encodeShare, LONG_URL, SHARE_PREFIX } from "./share";
import { LAB_LANGUAGES, languageForFile, languageInfo, starterFor } from "./starters";
import { StatusBar, type CursorSink } from "./StatusBar";
import {
  activeTab,
  addTab,
  adoptShared,
  closeTab,
  initialTabs,
  parseTabs,
  selectTab,
  setLanguage,
  setSource,
  type Tabs,
} from "./tabs";
import { tidy } from "./tidy";

/** The autosave can lag the keystroke it belongs to; it is a convenience, not a document. */
const SAVE_DEBOUNCE_MS = 800;
const DEFAULT_EDITOR_PCT = 55;
const DEFAULT_INPUT_PCT = 34;
const TOAST_MS = 2400;

const known = (language: string) => LAB_LANGUAGES.some((info) => info.id === language);
const OPENABLE = LAB_LANGUAGES.map((info) => `.${info.extension}`).join(",") + ",.h,.cc,.cxx,.hpp,.mjs,.cjs,.txt";

function loadTabs(): Tabs {
  try {
    const raw: unknown = JSON.parse(storageGet(CODE_LAB_TABS_KEY) ?? "null");
    return parseTabs(raw, storageGet(CODE_LAB_ACTIVE_KEY), known) ?? initialTabs();
  } catch {
    return initialTabs();
  }
}

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

export function CodeLab() {
  const [tabs, setTabsState] = useState<Tabs>(loadTabs);
  const tabsRef = useRef(tabs);
  const [stdin, setStdin] = useState(() => storageGet(CODE_LAB_STDIN_KEY) ?? "");
  const stdinRef = useRef(stdin);
  stdinRef.current = stdin;
  const [settings, setSettings] = useState<LabSettings>(() => parseSettings(storageGet(CODE_LAB_SETTINGS_KEY)));
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const [runState, setRunState] = useState<RunState>({ kind: "idle" });
  const running = useRef(false);
  const [copied, setCopied] = useState(false);
  const [full, setFull] = useState(false);

  const host = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const editor = useRef<EditorHandle | null>(null);
  const cursorSink = useRef<CursorSink | null>(null);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const columns = useLabSplitter(CODE_LAB_PANE_KEY, DEFAULT_EDITOR_PCT);
  const rows = useLabSplitter(CODE_LAB_IO_PANE_KEY, DEFAULT_INPUT_PCT, "y");

  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const say = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current != null) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), TOAST_MS);
  }, []);

  // ── persistence ──
  const saveNow = useCallback(() => {
    if (saveTimer.current != null) clearTimeout(saveTimer.current);
    saveTimer.current = null;
    storageSet(CODE_LAB_TABS_KEY, JSON.stringify(tabsRef.current.list));
    storageSet(CODE_LAB_ACTIVE_KEY, tabsRef.current.active);
    storageSet(CODE_LAB_STDIN_KEY, stdinRef.current);
  }, []);
  const scheduleSave = useCallback(() => {
    if (saveTimer.current != null) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(saveNow, SAVE_DEBOUNCE_MS);
  }, [saveNow]);

  useEffect(() => {
    scheduleSave();
  }, [stdin, scheduleSave]);

  // A closed tab must not lose the last keystrokes to the debounce.
  useEffect(() => {
    const flush = () => {
      if (saveTimer.current != null) saveNow();
    };
    const onHidden = () => {
      if (document.visibilityState === "hidden") flush();
    };
    window.addEventListener("pagehide", flush);
    document.addEventListener("visibilitychange", onHidden);
    return () => {
      window.removeEventListener("pagehide", flush);
      document.removeEventListener("visibilitychange", onHidden);
    };
  }, [saveNow]);

  /** Make `next` the lab's tabs, and bring the editor in line with the tab now on screen. */
  const apply = useCallback(
    (next: Tabs) => {
      tabsRef.current = next;
      setTabsState(next);
      scheduleSave();
      const tab = activeTab(next);
      const handle = editor.current;
      if (handle == null) return;
      handle.setLanguage(tab.language);
      if (handle.getValue() !== tab.source) handle.setValue(tab.source);
    },
    [scheduleSave],
  );

  // ── running ──
  const run = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    const tab = activeTab(tabsRef.current);
    const source = editor.current?.getValue() ?? tab.source;
    setRunState({ kind: "running", language: tab.language });
    log.info(`code lab: run ${tab.language} (${source.length} chars, ${stdinRef.current.length} of input)`);
    try {
      const result = await runCode({ language: tab.language, source, stdin: stdinRef.current });
      log.info(`code lab: ${tab.language} run finished: ${result.status}`);
      setRunState({ kind: "done", result, language: tab.language });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      log.warn(`code lab: run refused: ${message}`);
      setRunState({ kind: "failed", message });
    } finally {
      running.current = false;
    }
  }, []);
  const runRef = useRef(run);
  runRef.current = run;

  const copyCode = useCallback(async () => {
    if (!(await copyText(editor.current?.getValue() ?? activeTab(tabsRef.current).source))) {
      say("The clipboard is not available here.");
      return;
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1400);
  }, [say]);
  const copyRef = useRef(copyCode);
  copyRef.current = copyCode;

  const save = useCallback(() => {
    saveNow();
    say("Saved in this browser.");
  }, [saveNow, say]);
  const saveRef = useRef(save);
  saveRef.current = save;

  // ── the editor: mounted once, fed through refs so its handlers always see the current page ──
  useEffect(() => {
    const container = host.current;
    if (container == null) return;
    let gone = false;
    const tab = activeTab(tabsRef.current);
    const look = settingsRef.current;
    mountCodeLabEditor(container, {
      value: tab.source,
      language: tab.language,
      readOnly: false,
      dark: isDark(),
      ...look,
      onChange: (value) => {
        tabsRef.current = setSource(tabsRef.current, tabsRef.current.active, value);
        scheduleSave();
      },
      onRun: () => void runRef.current(),
      onCopy: () => void copyRef.current(),
      onSave: () => saveRef.current(),
      onCursor: (line, column, lines) => cursorSink.current?.(line, column, lines),
    })
      .then((handle) => {
        if (gone) return handle.dispose();
        editor.current = handle;
        // A share link may have changed the tabs while the chunk was loading.
        apply(tabsRef.current);
        handle.focus();
        log.info("code lab: editor ready");
      })
      .catch((error: unknown) => log.error(`code lab: editor failed to load: ${String(error)}`));
    return () => {
      gone = true;
      editor.current?.dispose();
      editor.current = null;
    };
  }, [apply, scheduleSave]);

  // The Default theme follows the site's light/dark switch; a named theme stays put.
  useEffect(() => {
    const follow = () => {
      if (settingsRef.current.theme === "default") editor.current?.applySettings({ theme: "default" });
    };
    window.addEventListener(THEME_CHANGED, follow);
    return () => window.removeEventListener(THEME_CHANGED, follow);
  }, []);

  // ── a share link: opened beside the reader's tabs, then taken off the URL so a reload does not
  //    open it a second time over whatever they did next. A link pasted into a lab that is
  //    already open changes only the fragment, which loads nothing, so `hashchange` opens it too ──
  useEffect(() => {
    const open = () => {
      const hash = window.location.hash;
      if (!hash.startsWith(SHARE_PREFIX)) return;
      history.replaceState(null, "", window.location.pathname + window.location.search);
      void decodeShare(hash).then((shared) => {
        if (shared == null) {
          log.warn("code lab: share link did not decode");
          say("This share link is damaged — nothing was opened.");
          return;
        }
        const usable = shared.tabs.filter((tab) => known(tab.language));
        log.debug(`code lab: opened a share link with ${usable.length} tab(s)`);
        if (usable.length === 0) return say("This share link holds no language the lab runs.");
        apply(adoptShared(tabsRef.current, usable, Math.min(shared.active, usable.length - 1)));
        setStdin(shared.stdin);
        say(`Opened ${usable.length === 1 ? "a shared program" : `${usable.length} shared programs`}.`);
      });
    };
    open();
    window.addEventListener("hashchange", open);
    return () => window.removeEventListener("hashchange", open);
  }, [apply, say]);

  // ── fullscreen: the whole lab, because the editor alone would hide the output it is run for ──
  useEffect(() => {
    const sync = () => setFull(document.fullscreenElement === root.current);
    document.addEventListener("fullscreenchange", sync);
    return () => document.removeEventListener("fullscreenchange", sync);
  }, []);
  const toggleFull = () => {
    if (document.fullscreenElement != null) void document.exitFullscreen();
    else void root.current?.requestFullscreen?.().catch(() => say("Fullscreen is not available here."));
  };

  // ── toolbar and file actions ──
  const tab = activeTab(tabs);
  const info = languageInfo(tab.language);

  const changeSettings = (next: LabSettings) => {
    setSettings(next);
    storageSet(CODE_LAB_SETTINGS_KEY, JSON.stringify(next));
    editor.current?.applySettings(next);
    log.debug(`code lab: settings ${JSON.stringify(next)}`);
  };

  const reset = () => {
    const current = activeTab(tabsRef.current);
    if (current.source !== starterFor(current.language) && !window.confirm(`Replace this tab's code with the ${info.label} starter?`)) return;
    apply(setSource(tabsRef.current, current.id, starterFor(current.language)));
  };

  const format = () => {
    const current = activeTab(tabsRef.current);
    const source = editor.current?.getValue() ?? current.source;
    const tidied = tidy(source, current.language, settingsRef.current.tabSize);
    if (tidied === source) return say("Already tidy.");
    // One undoable edit: Monaco's `onChange` records it into the tab and schedules the save, and
    // ⌘Z takes the reader back to what they had.
    if (editor.current) editor.current.replaceAll(tidied);
    else apply(setSource(tabsRef.current, current.id, tidied));
    say("Tidied the whitespace. ⌘Z undoes it.");
  };

  const download = () => {
    const current = activeTab(tabsRef.current);
    downloadText(fileNameFor(current.name, languageInfo(current.language).extension), current.source);
  };

  const open = async () => {
    try {
      const file = await pickTextFile(OPENABLE);
      if (file == null) return;
      const language = languageForFile(file.name) ?? activeTab(tabsRef.current).language;
      const name = file.name.replace(/\.[^.]+$/, "");
      apply(addTab(tabsRef.current, language, file.text, name));
      log.info(`code lab: opened ${file.name} as ${language}`);
    } catch (error) {
      say(error instanceof Error ? error.message : "Could not read that file.");
    }
  };

  const share = async () => {
    const list = tabsRef.current.list;
    const at = list.findIndex((item) => item.id === tabsRef.current.active);
    const hash = await encodeShare({ tabs: list, active: Math.max(at, 0), stdin: stdinRef.current });
    const url = `${window.location.origin}${window.location.pathname}${hash}`;
    if (!(await copyText(url))) return say("The clipboard is not available here.");
    say(url.length > LONG_URL ? "Share link copied — it is long, and some apps cut long links." : "Share link copied.");
  };

  const openInput = async () => {
    try {
      const file = await pickTextFile();
      if (file != null) setStdin(file.text);
    } catch (error) {
      say(error instanceof Error ? error.message : "Could not read that file.");
    }
  };

  const busy = runState.kind === "running";

  return (
    <div class="clab" ref={root}>
      <div class="lab-panes" ref={columns.panes}>
        <section class="lab-pane lab-pane--l clab__code" style={{ width: `${columns.leftPct}%` }}>
          <div class="clab-tabs" role="tablist" aria-label="Open programs">
            {tabs.list.map((item) => (
              <div class={`clab-tab${item.id === tabs.active ? " clab-tab--on" : ""}`}>
                <button
                  type="button"
                  role="tab"
                  class="clab-tab__name"
                  aria-selected={item.id === tabs.active}
                  title={`${item.name} · ${languageInfo(item.language).label}`}
                  onClick={() => apply(selectTab(tabsRef.current, item.id))}
                >
                  {item.name}
                </button>
                <button
                  type="button"
                  class="clab-tab__close"
                  aria-label={`Close ${item.name}`}
                  title={`Close ${item.name}`}
                  onClick={() => apply(closeTab(tabsRef.current, item.id))}
                >
                  {CROSS}
                </button>
              </div>
            ))}
            <button
              type="button"
              class="clab-tabs__add"
              aria-label="New tab"
              title="New tab"
              onClick={() => apply(addTab(tabsRef.current, activeTab(tabsRef.current).language))}
            >
              {PLUS}
            </button>
          </div>

          <div class="pane-hd clab-bar">
            <LanguageMenu value={tab.language} onChange={(language) => apply(setLanguage(tabsRef.current, tabsRef.current.active, language))} />
            <span class="pane-hd__sp"></span>
            <button type="button" class="pane-hd__btn" aria-label="Reset to the starter" title="Reset to the starter" onClick={reset}>
              {RESET}
            </button>
            <button type="button" class="pane-hd__btn" aria-label="Tidy whitespace" title="Tidy whitespace" onClick={format}>
              {FORMAT}
            </button>
            <button type="button" class="pane-hd__btn" aria-label="Copy editor code (⌥C)" title="Copy editor code (⌥C)" onClick={() => void copyCode()}>
              {copied ? TICK : COPY}
            </button>
            <button type="button" class="lab-primary clab-run" disabled={busy} title="Run (⌘⏎)" onClick={() => void run()}>
              {PLAY}
              <span>{busy ? "Running…" : "Run"}</span>
            </button>
            <SettingsPopover value={settings} onChange={changeSettings} />
            <button
              type="button"
              class="pane-hd__btn"
              aria-label={full ? "Leave fullscreen" : "Fullscreen"}
              title={full ? "Leave fullscreen" : "Fullscreen"}
              onClick={toggleFull}
            >
              {full ? SHRINK : EXPAND}
            </button>
          </div>

          {/* Monaco owns everything inside this node. */}
          <div class="clab__editor" ref={host} data-codelab-editor></div>

          <StatusBar
            sink={cursorSink}
            tabSize={settings.tabSize}
            onDownload={download}
            onOpen={() => void open()}
            onSave={save}
            onShare={() => void share()}
          />
        </section>

        <LabSplit label="Resize the editor" onPointerDown={columns.startDrag} />

        <section class="lab-pane lab-pane--r clab__io" ref={rows.panes}>
          <div class="clab-io clab-io--in" style={{ height: `${rows.leftPct}%` }}>
            <div class="pane-hd">
              <label class="pane-hd__eyebrow" for="clab-stdin">
                Input
              </label>
              <span class="pane-hd__sp"></span>
              <button type="button" class="pane-hd__btn" aria-label="Load input from a file" title="Load input from a file" onClick={() => void openInput()}>
                {UPLOAD}
              </button>
              <button type="button" class="pane-hd__btn" aria-label="Copy input" title="Copy input" onClick={() => void copyText(stdin).then((ok) => say(ok ? "Input copied." : "The clipboard is not available here."))}>
                {COPY}
              </button>
              <button type="button" class="pane-hd__btn" aria-label="Clear input" title="Clear input" onClick={() => setStdin("")}>
                {CLEAR}
              </button>
            </div>
            <textarea
              id="clab-stdin"
              class="clab-io__stdin"
              placeholder="Enter input..."
              spellcheck={false}
              value={stdin}
              onInput={(event) => setStdin((event.target as HTMLTextAreaElement).value)}
            ></textarea>
          </div>

          <LabSplit label="Resize the input" onPointerDown={rows.startDrag} axis="y" />

          <div class="clab-io clab-io--out">
            <div class="pane-hd">
              <span class="pane-hd__eyebrow">Output</span>
              <span class="pane-hd__sp"></span>
              <button
                type="button"
                class="pane-hd__btn"
                aria-label="Download output"
                title="Download output"
                disabled={outputText(runState) === ""}
                onClick={() => downloadText("output.txt", outputText(runState))}
              >
                {DOWNLOAD}
              </button>
              <button
                type="button"
                class="pane-hd__btn"
                aria-label="Copy output"
                title="Copy output"
                disabled={outputText(runState) === ""}
                onClick={() => void copyText(outputText(runState)).then((ok) => say(ok ? "Output copied." : "The clipboard is not available here."))}
              >
                {COPY}
              </button>
            </div>
            <div class="clab-out" data-codelab-output>
              <Output state={runState} />
            </div>
          </div>
        </section>
      </div>

      {toast != null && <div class="lab-toast">{toast}</div>}
    </div>
  );
}

const mount = document.querySelector<HTMLElement>("[data-codelab-root]");
if (mount != null) {
  render(
    h(LabGate, {
      title: "The code lab",
      blurb: "Write a program in any of eleven languages, give it input, and run it in the sandbox.",
      children: h(CodeLab, {}),
    }),
    mount,
  );
  log.info("code lab mounted");
}
