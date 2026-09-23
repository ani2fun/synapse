/**
 * `/viz` — the visualisation lab. The code is the input and the diagram is the output, so the
 * sides are the mirror of `/d2`: the canvas on the left, the real workbench on the right.
 *
 * THE LEFT PANE DRAWS; THE RIGHT PANE READS. Everything that describes the code rather than the
 * data — the call stack, the program's output, the stdin, and the prompt a waiting program has
 * stopped at — sits under the editor, because that is what a reader has their eyes on while
 * stepping. The wasm therefore mounts TWICE, canvas and console, over one store: two surfaces,
 * one run, no state crossing this page.
 *
 * The page owns the LIVE PAIR — the buffer and the stdin — and nothing else. It reads the buffer
 * off the workbench's own `synapse:code-changed` (the same seam the coach pane uses) rather than
 * holding a second copy, and hands (source, stdin, structure) to the wasm panel on Trace.
 *
 * The one thing it does mediate is the DEBUGGER CURSOR: the crate says which line just executed
 * and which is about to, and the page paints them onto the workbench's Monaco. Neither side could
 * do that alone — the crate does not own the editor, and the page does not know what a step is.
 *
 * The workbench is mounted IMPERATIVELY into its slot, as `islands/problem` does: its `root` is
 * the event surface, and Preact renders an empty node for it to fill so the two never fight over
 * one element. The canvas and console hosts are the same trick for the wasm.
 *
 * Everything stays in the tab. The buffer, the stdin and the structure choice autosave to
 * localStorage; nothing here is published anywhere.
 */
import { h, render } from "preact";
import { useCallback, useEffect, useMemo, useRef, useState } from "preact/hooks";

import * as log from "../../lib/log";
import {
  DEFAULT_LEFT_PCT,
  MAX_LEFT_PCT,
  MIN_LEFT_PCT,
  parseLeftPct,
  serializeLeftPct,
} from "../../lib/catalog/pane";
import type { Variant } from "../../lib/execution/blocks";
import type { EditorHandle } from "../../lib/islands/editor/monaco";
import {
  D2_BLANK_DRAFT_KEY,
  VIZ_LAB_DRAFT_PREFIX,
  VIZ_LAB_HINT_KEY,
  VIZ_LAB_PANE_KEY,
  VIZ_LAB_STDIN_KEY,
  get as storageGet,
  set as storageSet,
} from "../../lib/storage";
import { ensureViz } from "../viz";
import { CODE_CHANGED, VIZ_READY } from "../workbench/contracts";
import type { CodeSnapshot } from "../workbench/contracts";
import { Workbench } from "../workbench/Workbench";
import { composeHint, paramsFromUrl, splitHint } from "./params";
import { LANGUAGES, STARTER_HINT, STARTERS } from "./starters";

/** The draft is a convenience, not a document; it can lag the keystroke it belongs to. */
const DRAFT_DEBOUNCE_MS = 800;

const draftKeyFor = (language: string): string => `${VIZ_LAB_DRAFT_PREFIX}:${language}`;

/** The eye, not a play triangle. Trace sits beside Run and both execute the buffer, so a second
 *  ▶ makes the pair read as one segmented control; the eye is what "Visualise" already wears in
 *  this same bar on a lesson, and it says the difference — Run gives you the answer, Trace lets
 *  you watch it being worked out. */
const WATCH = (
  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
    <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"></path>
    <circle cx="12" cy="12" r="3"></circle>
  </svg>
);

const CHEVRON = (
  <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
    <path d="m6 9 6 6 6-6"></path>
  </svg>
);

/** The fence a d2 export lands in. A walkthrough opts into the board viewer by its info string;
 *  a single board is an ordinary figure. */
function fenceFor(source: string, walkthrough: boolean): string {
  const meta = walkthrough ? "d2 boards" : "d2";
  return `\`\`\`${meta}\n${source.replace(/\n$/, "")}\n\`\`\`\n`;
}

// ─────────────────────────────────────────────────────────────────────────────
// THE PAGE
// ─────────────────────────────────────────────────────────────────────────────

export function VizLab() {
  // Read once: the page is opened on a structure or it is not, and navigating changes neither.
  const opened = useMemo(() => paramsFromUrl(window.location.search), []);
  const remembered = useMemo(() => splitHint(storageGet(VIZ_LAB_HINT_KEY) ?? ""), []);
  const seed = splitHint(STARTER_HINT);

  const [structure, setStructure] = useState(
    opened.structure ?? remembered.structure ?? seed.structure ?? "array",
  );
  const [root, setRoot] = useState(opened.root ?? remembered.root ?? seed.root ?? "");
  const [stdin, setStdin] = useState(() => storageGet(VIZ_LAB_STDIN_KEY) ?? "");
  /** The vocabulary the crate can draw. Empty until the wasm lands — the picker says so. */
  const [structures, setStructures] = useState<string[]>([]);

  const canvasHost = useRef<HTMLDivElement>(null);
  const consoleHost = useRef<HTMLDivElement>(null);
  const benchSlot = useRef<HTMLDivElement>(null);
  const panes = useRef<HTMLDivElement>(null);
  /** The live Monaco, while there is one — the workbench evicts it off-viewport. */
  const editor = useRef<EditorHandle | null>(null);
  /** The last cursor the crate reported, so a Monaco that mounts LATE (or remounts after an
   *  eviction) is painted with where the reader actually is rather than with nothing. */
  const cursor = useRef<[number | null, number | null]>([null, null]);
  const dragging = useRef(false);
  /** The workbench's live buffer, off its own CODE_CHANGED — never a second copy of the code. */
  const live = useRef<CodeSnapshot>({
    source: STARTERS[LANGUAGES[0]] ?? "",
    language: LANGUAGES[0],
  });
  /** Read at Trace time, so the button's one handler always sees the current box. */
  const stdinRef = useRef(stdin);
  stdinRef.current = stdin;
  /** What a Run that stopped for input was asking — `""` when it asked without words — or null
   *  when no run is waiting. While set, the STDIN area IS the prompt. */
  const [asking, setAsking] = useState<string | null>(null);
  const [answer, setAnswer] = useState("");
  const answerBox = useRef<HTMLInputElement>(null);
  /** Starts a Run as the workbench's own button does; handed over once it mounts. */
  const runner = useRef<(() => void) | null>(null);
  /** The code the canvas is showing a run of, so an edit can take a now-wrong trace away. */
  const traced = useRef<CodeSnapshot | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const say = useCallback((message: string) => {
    setToast(message);
    setTimeout(() => setToast(null), 2400);
  }, []);

  const variants = useMemo<Variant[]>(
    () =>
      LANGUAGES.map((language) => ({
        language,
        source: storageGet(draftKeyFor(language)) ?? STARTERS[language] ?? "",
        // No hint: the STRUCTURE lives in the left pane here, and a hint would grow the
        // workbench a Visualise button that opens the modal over this very page.
        viz: null,
      })),
    [],
  );

  const readStdin = useCallback(() => stdinRef.current, []);

  const paintCursor = useCallback(() => {
    editor.current?.setLineHighlights(cursor.current[0], cursor.current[1]);
  }, []);

  // ── the workbench, mounted once into its slot ──
  // Outside Preact's tree on purpose: the workbench's `root` is its event surface, and the two
  // must never diff the same element.
  useEffect(() => {
    const slot = benchSlot.current;
    if (slot == null) return;
    const wrap = document.createElement("div");
    slot.replaceChildren(wrap);
    const onCode = (event: Event) => {
      const snapshot = (event as CustomEvent<CodeSnapshot>).detail;
      live.current = snapshot;
      // An edit ends the last run: its question and its trace both describe code that is gone.
      // PythonTutor does the same the moment you choose to edit — a trace left standing paints
      // its arrows onto the wrong lines (the last line, once the program got shorter).
      setAsking(null);
      const was = traced.current;
      if (was != null && (was.source !== snapshot.source || was.language !== snapshot.language)) {
        traced.current = null;
        window.__synapseVizPanel?.clear();
      }
    };
    wrap.addEventListener(CODE_CHANGED, onCode);
    render(
      h(Workbench, {
        variants,
        spec: null,
        lessonPath: [],
        root: wrap,
        practice: true,
        fill: true,
        editable: true,
        stdin: readStdin,
        onEditor: (handle) => {
          editor.current = handle;
          paintCursor();
        },
        onNeedsInput: setAsking,
        onRunner: (run) => {
          runner.current = run;
        },
      }),
      wrap,
    );
    log.info(`viz lab: workbench mounted (${variants.map((v) => v.language).join("/")})`);
    return () => {
      wrap.removeEventListener(CODE_CHANGED, onCode);
      render(null, wrap);
    };
  }, []);


  // ── the wasm: the canvas in one pane, the console in the other, over one store ──
  useEffect(() => {
    let live = true;
    const mount = () => {
      const canvas = canvasHost.current;
      const console_ = consoleHost.current;
      const panel = window.__synapseVizPanel;
      if (!live || canvas == null || console_ == null || panel == null) return;
      panel.mount(canvas);
      panel.mountConsole(console_);
      panel.onCursor((executed, next) => {
        cursor.current = [executed, next];
        paintCursor();
      });
      setStructures(panel.structures());
    };
    window.addEventListener(VIZ_READY, mount);
    void ensureViz().catch((error: unknown) =>
      log.error(`viz lab: the visualiser failed to load — ${String(error)}`),
    );
    // VIZ_READY may have fired before this listener existed (a warm bundle resolves instantly).
    mount();
    return () => {
      live = false;
      window.removeEventListener(VIZ_READY, mount);
    };
  }, []);

  // ── the drafts ──
  useEffect(() => {
    const timer = setTimeout(() => {
      storageSet(draftKeyFor(live.current.language), live.current.source);
    }, DRAFT_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });
  useEffect(() => {
    storageSet(VIZ_LAB_HINT_KEY, composeHint(structure, root));
  }, [structure, root]);
  useEffect(() => {
    const timer = setTimeout(() => storageSet(VIZ_LAB_STDIN_KEY, stdin), DRAFT_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [stdin]);

  // ── the splitter ──
  const [leftPct, setLeftPct] = useState(() => parseLeftPct(storageGet(VIZ_LAB_PANE_KEY)));
  useEffect(() => {
    const onMove = (event: PointerEvent) => {
      const box = panes.current?.getBoundingClientRect();
      if (!dragging.current || box == null || box.width <= 0) return;
      const pct = ((event.clientX - box.left) / box.width) * 100;
      setLeftPct(Math.min(Math.max(pct, MIN_LEFT_PCT), MAX_LEFT_PCT));
    };
    // Persist on RELEASE — this fires for every window pointerup, so it is gated on `dragging`
    // rather than writing storage at pointer rate.
    const onUp = () => {
      if (!dragging.current) return;
      dragging.current = false;
      document.body.style.cursor = "";
      const left = panes.current?.querySelector<HTMLElement>(".lab-pane--l");
      const pct = parseFloat(left?.style.width ?? "") || DEFAULT_LEFT_PCT;
      storageSet(VIZ_LAB_PANE_KEY, serializeLeftPct(pct));
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, []);

  const trace = () => {
    const panel = window.__synapseVizPanel;
    if (panel == null) {
      log.warn("viz lab: Trace pressed before the visualiser loaded");
      return;
    }
    const hint = composeHint(structure, root);
    log.info(`viz lab: trace ${live.current.language} as ${hint}`);
    traced.current = { ...live.current };
    panel.trace({
      language: live.current.language,
      source: live.current.source,
      vizHint: hint,
      stdin: stdinRef.current,
    });
  };

  // ── a Run that stopped to ask ──
  // The STDIN area becomes the prompt and TAKES focus: the program is stopped on a question, so
  // typing is the reader's next move, and making them find the box is how the page read as broken.
  useEffect(() => {
    if (asking == null) return;
    answerBox.current?.focus();
    answerBox.current?.scrollIntoView({ block: "nearest" });
  }, [asking]);

  /** The answer becomes the box's next line — so the box stays the one record of what the program
   *  is given — and the program runs on with it. */
  const answerRun = () => {
    const current = stdinRef.current;
    const next =
      current === "" ? answer : current.endsWith("\n") ? current + answer : `${current}\n${answer}`;
    // Run reads the box through this ref, and the state lands a render later — it must hold the
    // answer NOW, or the re-run is fed the box without it and asks the same question again.
    stdinRef.current = next;
    setStdin(next);
    setAnswer("");
    setAsking(null);
    log.info("viz lab: answered the program's question — running on");
    runner.current?.();
  };

  /** The traced figure as d2. Null when nothing is traced — the menu says so rather than
   *  copying an empty document. */
  const exportD2 = (mode: "step" | "walkthrough"): string | null =>
    window.__synapseVizPanel?.exportD2(mode) ?? null;

  const copyD2 = (mode: "step" | "walkthrough") => {
    setExportOpen(false);
    const source = exportD2(mode);
    if (source == null || source.trim() === "") {
      say("Trace something first — there is no figure to export yet.");
      return;
    }
    void navigator.clipboard.writeText(fenceFor(source, mode === "walkthrough")).then(
      () => say(mode === "walkthrough" ? "Walkthrough fence copied." : "d2 fence copied."),
      () => say("Could not reach the clipboard."),
    );
  };

  // Hands the figure to `/d2` through ITS blank-draft key (storage.ts names the coupling): the
  // editor opens on that draft, so the source is simply there — no URL payload, no new endpoint.
  const openInD2 = () => {
    setExportOpen(false);
    const source = exportD2("step");
    if (source == null || source.trim() === "") {
      say("Trace something first — there is no figure to open.");
      return;
    }
    storageSet(D2_BLANK_DRAFT_KEY, source);
    log.info("viz lab: handing the step to /d2");
    window.location.assign("/d2");
  };

  const ready = structures.length > 0;

  // ── the canvas's own verbs ──
  // Trace fills the canvas and the export copies what it drew, so they sit in the canvas header
  // beside the structure and root they read — and land on the same band as the editor's Run,
  // which is the other way to say "do something with this code".
  const canvasActions = (
    <>
      <div class="vlab__export">
        <button
          class="vlab__export-btn"
          aria-haspopup="menu"
          aria-expanded={exportOpen}
          title="Copy the traced figure as a d2 diagram"
          onClick={() => setExportOpen((open) => !open)}
        >
          Copy as d2
          {CHEVRON}
        </button>
        {exportOpen && (
          <div>
            <div class="vlab__export-scrim" onClick={() => setExportOpen(false)}></div>
            <div class="vlab__export-menu" role="menu">
              <button role="menuitem" onClick={() => copyD2("step")}>
                Copy this step
              </button>
              <button role="menuitem" onClick={() => copyD2("walkthrough")}>
                Copy walkthrough
              </button>
              <button role="menuitem" onClick={openInD2}>
                Open this step in /d2
              </button>
            </div>
          </div>
        )}
      </div>
      <button
        class="vlab__trace"
        onClick={trace}
        disabled={!ready}
        title={ready ? "Run this code and capture it after every line" : "Loading the visualiser…"}
      >
        {WATCH}
        Trace
      </button>
    </>
  );

  return (
    <div class="vlab">
      <header class="lab-doc">
        <div class="lab-doc__id">
          <span class="pane-hd__eyebrow">Visualisation lab</span>
          <h1 class="vlab__title">Watch your code run</h1>
          <p class="vlab__lede">
            We run it for real and capture the structure after every line — an actual run, not a
            simulation.
          </p>
        </div>
      </header>

      <div class="lab-panes" ref={panes}>
        <section class="lab-pane lab-pane--l" style={{ width: `${leftPct}%` }}>
          <div class="pane-hd">
            <span class="pane-hd__eyebrow">Canvas</span>
            <label class="vlab__pick">
              <span class="vlab__pick-label">Structure</span>
              <select
                class="vlab__select"
                disabled={!ready}
                value={structure}
                onChange={(event) => setStructure((event.target as HTMLSelectElement).value)}
              >
                {/* The remembered choice may name a structure this build cannot draw (an older
                    tab, a hand-edited URL). Offer it anyway rather than silently switching to
                    something else — Trace will say so honestly. */}
                {(structures.includes(structure) ? structures : [structure, ...structures]).map(
                  (token) => (
                    <option value={token}>{token}</option>
                  ),
                )}
              </select>
            </label>
            <label class="vlab__pick">
              <span class="vlab__pick-label">Root</span>
              <input
                class="vlab__root"
                placeholder="arr"
                title="The variable to watch — leave blank to let the adapter find it"
                value={root}
                onInput={(event) => setRoot((event.target as HTMLInputElement).value)}
              />
            </label>
            <span class="pane-hd__sp"></span>
            {canvasActions}
          </div>
          {/* Leptos owns everything inside this node. Preact must never render into it again. */}
          <div class="vlab__canvas" ref={canvasHost} data-vizlab-canvas></div>
        </section>
        <div
          class="lab-split"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize the canvas"
          onPointerDown={(event) => {
            event.preventDefault();
            dragging.current = true;
            document.body.style.cursor = "col-resize";
          }}
        >
          <span class="lab-split__grip">
            <i></i>
            <i></i>
            <i></i>
          </span>
        </div>
        <section class="lab-pane lab-pane--r">
          {/* Three empty nodes, three owners. Preact renders each once and never looks inside:
              the workbench renders itself into the first, Leptos into the second, and only the
              stdin box below is the page's own. */}
          <div class="vlab__bench" ref={benchSlot}></div>
          <div class="vlab__console" ref={consoleHost} data-vizlab-console></div>
          <div class="vlab__stdin">
            <label class="vlab__stdin-label" for="vlab-stdin">
              stdin
            </label>
            {/* The program's input, one line per `input()`. The sandbox is one-shot, so neither
                button can type into a RUNNING program; both ask instead. A Run that reaches the
                end of this box stops on the question, this area becomes the prompt, and the
                answer is added here as the next line before the program runs again from the top.
                Trace asks in the console as the reader steps, and those answers stay in its own
                session. */}
            {asking != null && (
              <div class="viz-input__ask vlab__ask">
                <label class="viz-input__label" for="vlab-answer">
                  {asking === "" ? "The program is waiting for input" : asking}
                </label>
                <div class="viz-input__row">
                  <input
                    id="vlab-answer"
                    ref={answerBox}
                    class="viz-input__box"
                    placeholder="Type a value, then press Enter"
                    value={answer}
                    onInput={(event) => setAnswer((event.target as HTMLInputElement).value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        answerRun();
                      }
                    }}
                  />
                  <button type="button" class="viz-input__go" onClick={answerRun}>
                    Run on
                  </button>
                </div>
              </div>
            )}
            <textarea
              id="vlab-stdin"
              class="vlab__stdin-input"
              rows={2}
              placeholder="One line per input the program reads"
              value={stdin}
              onInput={(event) => setStdin((event.target as HTMLTextAreaElement).value)}
            ></textarea>
            <p class="vlab__stdin-note">
              One line per <code>input()</code>, read in order. When the program wants more than
              the box holds, Run stops and asks — your answer is added here as the next line.
              Trace asks as you step.
            </p>
          </div>
        </section>
      </div>

      {toast != null && <div class="lab-toast">{toast}</div>}
    </div>
  );
}

const host = document.querySelector<HTMLElement>("[data-vizlab-root]");
if (host != null) {
  render(h(VizLab, {}), host);
  log.info("viz lab mounted");
}
