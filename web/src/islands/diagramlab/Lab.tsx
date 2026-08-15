/**
 * The diagram editor's shell — source on the left, the live figure on the right.
 *
 * Everything here is true of both `/d2` and `/mermaid`: the buffer, the diagram it was opened on,
 * the autosave, the split, the toolbar, and the route out through the authoring pipeline. What is
 * left over is per-language — what the fence's info string says, what the document is called, and
 * what the canvas draws — and that comes back from `useDoc`, a hook the adapter owns so its state
 * lives with the code that reads it.
 *
 * Everything stays in the tab. The source autosaves to localStorage and leaves only when the
 * author copies the fence, downloads it, or proposes it as an edit.
 */
import { type ComponentChildren, h } from "preact";
import { useCallback, useEffect, useMemo, useRef, useState } from "preact/hooks";

import { AddToLesson } from "./AddToLesson";
import { type DiagramLang, extensionOfLang, fencesOfLang } from "./lang";
import { type DiagramProblem } from "./PreviewFrame";
import { type Subject, subjectFromUrl } from "./subject";
import * as api from "../../lib/api/client";
import { Icon } from "./icons";
import {
  DIAGRAM_LAB_DRAFT_PREFIX,
  DIAGRAM_LAB_PANE_KEY,
  THEME_KEY,
  get as storageGet,
  set as storageSet,
} from "../../lib/storage";
import { type EditorHandle } from "../../lib/islands/editor/monaco";
import { download } from "../../lib/zip";
import * as log from "../../lib/log";
import { mountDiagramEditor } from "../../lib/islands/editor/loader";

/** The draft is a convenience, not a document; it can lag the keystroke it belongs to. */
const DRAFT_DEBOUNCE_MS = 800;

const DEFAULT_LEFT_PCT = 44;
const MIN_LEFT_PCT = 28;
const MAX_LEFT_PCT = 64;

function clampPct(value: number): number {
  if (!Number.isFinite(value) || value <= 0) return DEFAULT_LEFT_PCT;
  return Math.min(Math.max(value, MIN_LEFT_PCT), MAX_LEFT_PCT);
}

/** The draft key. Scoped per language AND per diagram, so editing two of them does not have one
 *  overwrite the other's autosave, and neither disturbs the blank scratchpad. */
const draftKeyFor = (lang: DiagramLang, subject: Subject | null): string =>
  subject == null
    ? `${DIAGRAM_LAB_DRAFT_PREFIX}:${lang}`
    : `${DIAGRAM_LAB_DRAFT_PREFIX}:${lang}:${subject.lessonPath}:${subject.at}`;

// ─────────────────────────────────────────────────────────────────────────────
// THE ADAPTER CONTRACT
// ─────────────────────────────────────────────────────────────────────────────

/** What the shell hands an adapter on every render. */
export interface LabContext {
  source: string;
  /**
   * Replace the whole buffer — for an edit the author made somewhere other than the editor, like
   * a title field whose value lives in the source. Goes THROUGH Monaco rather than around it: the
   * editor owns the buffer, and this keeps one undo step.
   */
  setSource: (next: string) => void;
  /** The diagram this page was opened on, or null for a blank draft. */
  subject: Subject | null;
  /** The info string the opened fence carried — the adapter decides what that means. */
  openedMeta: string | null;
  /** Flash a message in the toast strip. */
  say: (message: string) => void;
  /** Put the caret on a line of the source pane. */
  goToLine: (line: number) => void;
}

/** What an adapter hands back. */
export interface LabDoc {
  /** The fence's info string, appended after the language on the opening backticks. */
  meta: string;
  /** The buffer's name in the pane header and the Download button. */
  fileName: string;
  /** The directory a submit will create, named in the confirm step — or null when it creates
   *  none, in which case claiming one would describe a file the change never writes. */
  sidecar: string | null;
  /** The eyebrow's second half for a blank draft: "D2 walkthrough · draft". */
  draftLabel: string;
  /** The title + the metadata line under the eyebrow. */
  identity: ComponentChildren;
  /** Buttons that join Copy/Download in the segmented control. */
  actions?: ComponentChildren;
  /** The right pane, already wrapped in `PreviewFrame`. */
  preview: ComponentChildren;
}

// ─────────────────────────────────────────────────────────────────────────────
// THE PAGE
// ─────────────────────────────────────────────────────────────────────────────

export function Lab({
  lang,
  starter,
  useDoc,
  tidy,
}: {
  lang: DiagramLang;
  /** The blank draft's seed. */
  starter: string;
  useDoc: (ctx: LabContext) => LabDoc;
  /** Re-indent the buffer, for a language where that is meaningful. Omitted rather than
   *  no-op'd: a Tidy button that does nothing is worse than none. */
  tidy?: (source: string) => string;
}) {
  // Read once: the page is opened on a diagram or it is not, and navigating changes neither.
  const subject = useMemo(() => subjectFromUrl(window.location.search), []);
  const draftKey = draftKeyFor(lang, subject);
  const [source, setSource] = useState<string>(() =>
    // A blank page starts on its draft; a page opened on a diagram starts EMPTY and fills from
    // the lesson, so a stale draft cannot masquerade as what is actually published there.
    subject == null ? (storageGet(draftKey) ?? starter) : "",
  );
  /** What the lesson holds right now — the thing an update is checked against. */
  const [published, setPublished] = useState<string | null>(null);
  const [openedMeta, setOpenedMeta] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [leftPct, setLeftPct] = useState(() => clampPct(Number(storageGet(DIAGRAM_LAB_PANE_KEY))));
  const [toast, setToast] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const editorHost = useRef<HTMLDivElement>(null);
  const editor = useRef<EditorHandle | null>(null);
  const panes = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  /** The buffer as of now, for the mount below: it runs once and would otherwise capture the
   *  empty string a page opened on a diagram starts with. */
  const sourceRef = useRef(source);
  sourceRef.current = source;

  const say = useCallback((message: string) => {
    setToast(message);
    setTimeout(() => setToast(null), 2400);
  }, []);

  const goToLine = useCallback((line: number) => editor.current?.goToLine(line), []);

  /** Through the editor, not just state: it owns the buffer, and this keeps one undo step. */
  const replaceSource = useCallback((next: string) => {
    editor.current?.setValue(next);
    setSource(next);
  }, []);

  const doc = useDoc({ source, setSource: replaceSource, subject, openedMeta, say, goToLine });
  const fence = `\`\`\`${lang}${doc.meta === "" ? "" : ` ${doc.meta}`}\n${source.replace(/\n$/, "")}\n\`\`\`\n`;

  // Monaco mounts ONCE and pushes changes out; re-rendering it per keystroke would fight the
  // editor for the buffer it owns.
  useEffect(() => {
    const node = editorHost.current;
    if (node == null) return;
    let disposed = false;
    void mountDiagramEditor(
      node,
      sourceRef.current,
      lang,
      storageGet(THEME_KEY) === "dark",
      setSource,
    ).then(
      (mounted) => {
        if (disposed) mounted.dispose();
        else editor.current = mounted;
      },
      (error: unknown) => log.warn(`diagram lab: the editor failed to mount — ${String(error)}`),
    );
    return () => {
      disposed = true;
      editor.current?.dispose();
      editor.current = null;
    };
  }, []);

  /**
   * Load the diagram this page was opened on.
   *
   * From the PUBLIC lesson payload, so opening a diagram to look at it needs no sign-in — the
   * gate is on proposing a change, not on reading one. The authoritative file is fetched again at
   * submit time, where the fingerprint and the fence guard both apply.
   */
  useEffect(() => {
    if (subject == null) return;
    let live = true;
    void api.lesson(subject.lessonPath.split("/")).then(
      (payload) => {
        if (!live) return;
        const found = fencesOfLang(payload.raw, lang).slice(subject.at, subject.at + subject.count);
        if (found.length === 0) {
          setLoadError(`That lesson has no diagram at position ${subject.at + 1}.`);
          return;
        }
        // A run of adjacent fences is one figure; it edits as one source.
        const text = found.map((fence) => fence.source).join("\n");
        const opening = storageGet(draftKey) ?? text;
        setPublished(text);
        setSource(opening);
        editor.current?.setValue(opening);
        setOpenedMeta(found[0]!.meta);
      },
      (error: unknown) => {
        if (live) setLoadError(error instanceof Error ? error.message : String(error));
      },
    );
    return () => {
      live = false;
    };
  }, [subject]);

  useEffect(() => {
    if (source === "") return; // nothing to save before the diagram has loaded
    const timer = setTimeout(() => storageSet(draftKey, source), DRAFT_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [source, draftKey]);

  useEffect(() => {
    const onMove = (event: PointerEvent) => {
      const box = panes.current?.getBoundingClientRect();
      if (!dragging.current || box == null || box.width <= 0) return;
      setLeftPct(clampPct(((event.clientX - box.left) / box.width) * 100));
    };
    const onUp = () => {
      if (!dragging.current) return;
      dragging.current = false;
      document.body.style.cursor = "";
      const left = panes.current?.querySelector<HTMLElement>(".lab-pane--l");
      storageSet(
        DIAGRAM_LAB_PANE_KEY,
        (parseFloat(left?.style.width ?? "") || DEFAULT_LEFT_PCT).toFixed(2),
      );
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, []);

  // Always answers, because the commonest outcome is that the source was ALREADY tidy — and a
  // button that silently computes an identical string is indistinguishable from a broken one.
  const onTidy = () => {
    if (tidy == null) return;
    const tidied = tidy(source);
    if (tidied === source) {
      say("Already tidy.");
      return;
    }
    replaceSource(tidied);
    say("Tidied the indentation.");
  };

  return (
    <div class="dlab">
      <header class="lab-doc">
        <div class="lab-doc__id">
          <span class="pane-hd__eyebrow">
            {subject == null ? `${doc.draftLabel} · draft` : `Editing · ${subject.lessonPath}`}
          </span>
          {doc.identity}
          {loadError != null && <p class="lab-meta__bad">{loadError}</p>}
        </div>
        <div class="lab-acts">
          <div class="lab-seg">
            <button
              onClick={() =>
                void navigator.clipboard.writeText(fence).then(
                  () => say("Fence copied — paste it into a lesson."),
                  () => say("Could not reach the clipboard."),
                )
              }
            >
              <Icon name="copy" size={15} />
              Copy fence
            </button>
            <button
              onClick={() => {
                download(new Blob([source], { type: "text/plain" }), doc.fileName);
                say(`Downloaded ${doc.fileName}`);
              }}
            >
              <Icon name="download" size={15} />
              {`Download .${extensionOfLang(lang)}`}
            </button>
            {doc.actions}
          </div>
          {/* A diagram opened from a lesson goes BACK to that lesson; a blank draft has to be
              told where to go. Same pipeline either way. */}
          <button class="lab-primary" onClick={() => setAdding(true)} disabled={source === ""}>
            <Icon name={subject == null ? "plus" : "check"} size={15} />
            {subject == null ? "Add to a lesson…" : "Update the diagram"}
          </button>
        </div>
      </header>

      <div class="lab-panes" ref={panes}>
        <section class="lab-pane lab-pane--l" style={{ width: `${leftPct}%` }}>
          <div class="pane-hd">
            <span class="pane-hd__eyebrow">Source</span>
            <span class="pane-hd__file">{doc.fileName}</span>
            <span class="pane-hd__sp"></span>
            {tidy != null && (
              // The glyph alone reads as a menu, so the label is spelled out beside it — this is
              // the one control on the page whose effect is invisible until it has happened.
              <button
                class="pane-hd__btn pane-hd__btn--label"
                title="Re-indent the source — two spaces per level, nothing else changes"
                aria-label="Re-indent the source"
                onClick={onTidy}
              >
                <Icon name="tidy" size={15} />
                <span>Tidy</span>
              </button>
            )}
          </div>
          <div class="lab-ed" ref={editorHost}></div>
        </section>
        <div
          class="lab-split"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize the editor"
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
        <section class="lab-pane lab-pane--r">{doc.preview}</section>
      </div>

      {toast != null && (
        <div class="lab-toast">
          <Icon name="check" size={15} />
          {toast}
        </div>
      )}
      {adding && (
        <AddToLesson
          lang={lang}
          fence={fence}
          sidecar={doc.sidecar}
          subject={subject}
          published={published}
          onClose={() => setAdding(false)}
        />
      )}
    </div>
  );
}

/** Mount a lab onto the shell the page SSR'd, and say so in the log. */
export function mountLab(render: (root: HTMLElement) => void, what: string): void {
  const root = document.querySelector<HTMLElement>("[data-diagramlab-root]");
  if (root == null) return;
  render(root);
  log.info(`${what} mounted`);
}

export type { DiagramProblem };
