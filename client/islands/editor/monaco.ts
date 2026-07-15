// ──────────────────────────────────────────────────────────────────
// MONACO EDITOR ISLAND
// vanilla monaco-editor, set up once, behind the @editor lazy chunk
// ──────────────────────────────────────────────────────────────────
// The runnable code block (step 11) mounts a read-only Monaco here to show a
// snippet; editing unlocks with identity (Phase 6). This module is reached
// only through loader.ts's dynamic import(), so monaco's editor core + web
// worker land in their own on-demand chunk — never on the initial bundle.
//
// We pull the editor *core + all editor contributions* (edcore.main) plus Monarch
// tokenizers for the languages we run — NOT the ts/json/css/html *language services*
// — so a single editor web-worker is all monaco ever asks for.
//
// IMPORTANT: use `edcore.main`, not `editor.api`. `editor.api` is only the bare API
// surface — it registers NONE of the editor feature contributions, so multi-cursor
// (Cmd/Ctrl+D), find (Cmd/Ctrl+F), comment toggle (Cmd/Ctrl+/), line move (Alt+↑/↓),
// go-to-line, the command palette, etc. all silently do nothing. `edcore.main` pulls
// `editor.all.js` (every contribution) + the standalone quick-access features and
// re-exports the same API — the full VSCode editor keymap, minus the bundled
// languages (we still register only our own Monarch grammars below).

import * as monaco from "monaco-editor/esm/vs/editor/edcore.main";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";

// Syntax highlighting (Monarch) for the runnable languages — see the server's
// Language registry (execution/domain). C maps onto the cpp grammar.
import "monaco-editor/esm/vs/basic-languages/python/python.contribution";
import "monaco-editor/esm/vs/basic-languages/java/java.contribution";
import "monaco-editor/esm/vs/basic-languages/scala/scala.contribution";
import "monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution";
import "monaco-editor/esm/vs/basic-languages/go/go.contribution";
import "monaco-editor/esm/vs/basic-languages/rust/rust.contribution";
import "monaco-editor/esm/vs/basic-languages/kotlin/kotlin.contribution";
import "monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution";
import "monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution";
import "monaco-editor/esm/vs/basic-languages/sql/sql.contribution";

// One editor worker for every label — we don't load the language services that
// need dedicated workers, so the base worker is all monaco requests.
(self as unknown as { MonacoEnvironment: monaco.Environment }).MonacoEnvironment = {
  getWorker: () => new EditorWorker(),
};

// Fence alias → Monaco language id (they mostly match; a few differ / collapse).
const languageIds: Record<string, string> = {
  python: "python",
  java: "java",
  scala: "scala",
  c: "cpp",
  cpp: "cpp",
  "c++": "cpp",
  go: "go",
  rust: "rust",
  kotlin: "kotlin",
  typescript: "typescript",
  javascript: "javascript",
  sql: "sql",
};

function monacoLanguage(fenceLang: string): string {
  return languageIds[fenceLang.toLowerCase()] ?? "plaintext";
}

// Two themes on the Synapse palette (tokens live in tailwind.css; Monaco needs
// literal hexes, so these mirror --card). Picked by the reader's light/dark mode
// at creation time. Defined once, lazily.
let themesDefined = false;

function defineThemes(): void {
  if (themesDefined) return;
  monaco.editor.defineTheme("synapse-light", {
    base: "vs",
    inherit: true,
    rules: [],
    colors: { "editor.background": "#f5f2ea" }, // HelloInterview --code-bg (light)
  });
  monaco.editor.defineTheme("synapse-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [],
    colors: { "editor.background": "#181b21" }, // HelloInterview --code-bg (dark)
  });
  themesDefined = true;
}

export interface EditorHandle {
  dispose: () => void;
  setTheme: (dark: boolean) => void;
  /**
   * Toggle read-only in place (the ⌘E unlock) without recreating the editor — cursor and undo
   * history survive. (synapse-rs extension: the oracle handle had no in-place toggle.)
   */
  setReadOnly: (readOnly: boolean) => void;
  /** Replace the whole buffer (the workbench's Reset-to-starter; step 16). */
  setValue: (value: string) => void;
  /** Read the live buffer (the one-click copy overlay). */
  getValue: () => string;
  /**
   * Highlight the current step's line (and, when given, the upcoming one) and scroll it into view — the
   * Visualise modal's SourcePane (step 30), Python-Tutor style. `current`/`next` are 1-indexed source lines.
   */
  setLineHighlights: (current: number, next: number | null) => void;
}

export interface EditorOptions {
  value: string;
  language: string; // fence alias
  readOnly: boolean;
  dark: boolean;
  /** Fires with the full buffer on every edit — problem workbenches feed their code state from it (step 16). */
  onChange?: (value: string) => void;
  /** Workbench keymap (VS Code muscle memory on our own verbs) — wired only where the surface has the verb. */
  onRun?: () => void; // Cmd/Ctrl+Enter — run the block
  onSubmit?: () => void; // Cmd/Ctrl+Shift+Enter — submit against the hidden suite (problems)
  onToggleEdit?: () => void; // Cmd/Ctrl+E — toggle the page-local Edit unlock (lesson blocks)
}

/** Create a Monaco editor in `container`; returns a dispose handle for unmount. */
export function createEditor(container: HTMLElement, opts: EditorOptions): EditorHandle {
  defineThemes();
  const editor = monaco.editor.create(container, {
    value: opts.value,
    language: monacoLanguage(opts.language),
    readOnly: opts.readOnly,
    domReadOnly: opts.readOnly,
    theme: opts.dark ? "synapse-dark" : "synapse-light",
    automaticLayout: true, // monaco's own ResizeObserver — no manual layout dance
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    lineNumbers: "on",
    fontFamily: '"JetBrains Mono", ui-monospace, monospace',
    fontSize: 13,
    lineHeight: 20,
    tabSize: 4,
    padding: { top: 12, bottom: 12 },
    // Let the page keep scrolling once the editor's own scroll is exhausted.
    scrollbar: { alwaysConsumeMouseWheel: false },
    overviewRulerLanes: 0,
    renderLineHighlight: opts.readOnly ? "none" : "line",
    // ── Harness Monaco's built-in richness (it IS VSCode's editor core, so most VSCode keybindings work as
    //    they do there — multi-cursor Cmd/Ctrl+D, line move Alt+↑/↓, comment toggle Cmd/Ctrl+/, find Cmd/Ctrl+F,
    //    the command palette F1, go-to-line, etc.). We only turn OFF what's genuinely noise at prose width. ──
    contextmenu: !opts.readOnly, // right-click menu (shows the actions + their keybindings) on editable blocks
    folding: true, // code folding + the fold gutter
    bracketPairColorization: { enabled: true },
    matchBrackets: "always",
    multiCursorModifier: "ctrlCmd", // VSCode's default (Cmd/Ctrl+Click adds a cursor)
    quickSuggestions: !opts.readOnly, // word-based IntelliSense as you type (editable only)
    suggestOnTriggerCharacters: !opts.readOnly,
    wordBasedSuggestions: opts.readOnly ? "off" : "currentDocument",
    tabCompletion: "on",
    autoClosingBrackets: "languageDefined",
    autoIndent: "full",
    formatOnPaste: true,
    cursorBlinking: "smooth",
    smoothScrolling: true,
    stickyScroll: { enabled: false }, // cramped at ~700px prose width; the fence stays scannable without it
  });
  if (opts.onChange) {
    const notify = opts.onChange;
    editor.onDidChangeModelContent(() => notify(editor.getValue()));
  }
  // ── The workbench keymap. `addAction` (not `addCommand`): actions surface in the right-click menu and the
  //    F1 command palette with their keybinding shown, so the shortcuts are discoverable, not folklore. They
  //    fire in read-only editors too — Run works on the read-only starter, and Cmd/Ctrl+E is exactly how a
  //    locked editor becomes editable. Everything else (multi-cursor Cmd/Ctrl+D, find Cmd/Ctrl+F, line move
  //    Alt+↑/↓, the palette F1, …) is already VSCode's own keymap via edcore.main — see the header note. ──
  if (opts.onRun) {
    const run = opts.onRun;
    editor.addAction({
      id: "synapse.run",
      label: "Run code",
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter],
      contextMenuGroupId: "1_synapse",
      contextMenuOrder: 1,
      run: () => run(),
    });
  }
  if (opts.onSubmit) {
    const submit = opts.onSubmit;
    editor.addAction({
      id: "synapse.submit",
      label: "Submit solution",
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.Enter],
      contextMenuGroupId: "1_synapse",
      contextMenuOrder: 2,
      run: () => submit(),
    });
  }
  if (opts.onToggleEdit) {
    const toggle = opts.onToggleEdit;
    editor.addAction({
      id: "synapse.toggleEdit",
      label: "Toggle editing",
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyE],
      contextMenuGroupId: "1_synapse",
      contextMenuOrder: 3,
      run: () => toggle(),
    });
  }
  // One collection, replaced wholesale on every step (SourcePane calls this once per step change).
  const highlights = editor.createDecorationsCollection([]);
  return {
    dispose: () => editor.dispose(),
    // monaco.editor.setTheme is GLOBAL (re-themes every editor) — cheap + idempotent. The reader's theme
    // toggle calls this so the editor recolors immediately instead of keeping its creation-time theme
    // until the next full reload.
    setTheme: (dark: boolean) => monaco.editor.setTheme(dark ? "synapse-dark" : "synapse-light"),
    setReadOnly: (readOnly: boolean) => {
      editor.updateOptions({
        readOnly,
        domReadOnly: readOnly,
        renderLineHighlight: readOnly ? "none" : "line",
        contextmenu: !readOnly,
        quickSuggestions: !readOnly,
        suggestOnTriggerCharacters: !readOnly,
        wordBasedSuggestions: readOnly ? "off" : "currentDocument",
      });
    },
    setValue: (value: string) => editor.setValue(value),
    getValue: () => editor.getValue(),
    setLineHighlights: (current: number, next: number | null) => {
      const decos: monaco.editor.IModelDeltaDecoration[] = [
        {
          range: new monaco.Range(current, 1, current, 1),
          options: {
            isWholeLine: true,
            className: "wb-source-current-line",
            linesDecorationsClassName: "wb-source-current-gutter",
          },
        },
      ];
      if (next != null) {
        decos.push({
          range: new monaco.Range(next, 1, next, 1),
          options: { isWholeLine: true, className: "wb-source-next-line" },
        });
      }
      highlights.set(decos);
      editor.revealLineInCenterIfOutsideViewport(current);
    },
  };
}
