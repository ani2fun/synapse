/**
 * `/d2` — the diagram editor. Source on the left, the live walkthrough on the right.
 *
 * The document's identity IS the page title: the root board's name is an editable heading rather
 * than a labelled field, with the sidecar path, the line count and the save state as one quiet
 * line of metadata under it. The compile state is a single pill that changes colour instead of
 * three different layouts, so the toolbar never reflows between "drawing" and "done".
 *
 * The preview is not a picture of the diagram — it mounts the reader's own board walk, so
 * clicking a node drills down exactly as it will in a lesson. The board control floats over the
 * canvas rather than sitting under the figure, because on this page the canvas is the subject.
 *
 * Everything stays in the tab. The source autosaves to localStorage and leaves only when the
 * author copies the fence, downloads it, or proposes it as an edit.
 */
import { h, render } from "preact";
import { useCallback, useEffect, useMemo, useRef, useState } from "preact/hooks";

import { AddToLesson } from "./AddToLesson";
import * as api from "../../lib/api/client";
import { Icon } from "./icons";
import {
  D2_LAB_DRAFT_KEY,
  D2_LAB_PANE_KEY,
  THEME_KEY,
  get as storageGet,
  set as storageSet,
} from "../../lib/storage";
import { type BoardProvider, BoardBar, engineProvider, useBoardWalk } from "../widgets/D2Boards";
import { ZoomOverlay } from "../widgets/Zoom";
import { type D2Problem, firstProblem } from "../../lib/islands/diagram/d2Errors";
import { type EditorHandle } from "../../lib/islands/editor/monaco";
import { download, zip } from "../../lib/zip";
import {
  boardsDirName,
  fenceName,
  isBoardsFence,
  rootTitleOf,
} from "../../lib/islands/diagram/boards";
import { d2Fences } from "../../lib/markdown/fences";
import { fnv1a } from "../../lib/hash";
import * as log from "../../lib/log";
import { mountD2Editor } from "../../lib/islands/editor/loader";

/** Long enough that a burst of typing is one compile, short enough to feel live. */
const COMPILE_DEBOUNCE_MS = 450;
/** The draft is a convenience, not a document; it can lag the keystroke it belongs to. */
const DRAFT_DEBOUNCE_MS = 800;

const DEFAULT_LEFT_PCT = 44;
const MIN_LEFT_PCT = 28;
const MAX_LEFT_PCT = 64;

const STARTER = `# A walkthrough: click a linked node to drill in.
direction: right

user: "Link creator" {
  shape: person
}
shortener: "URL Shortener" {
  link: layers.container
}
user -> shortener: Creates short links

layers: {
  container: {
    api: "Public API" {
      link: _.layers.component
    }
    store: "URL mappings" {
      shape: cylinder
    }
    api -> store: Reads and writes
  }
  component: {
    handler: "Redirect Handler"
    cache: "Mapping Cache"
    handler -> cache: Lookup
  }
}
`;

function clampPct(value: number): number {
  if (!Number.isFinite(value) || value <= 0) return DEFAULT_LEFT_PCT;
  return Math.min(Math.max(value, MIN_LEFT_PCT), MAX_LEFT_PCT);
}

/**
 * A diagram this page was opened ON, from `?lesson=&at=&count=` — the Edit pill on a lesson's
 * figure. Absent means a blank draft, which is what `/d2` has always been.
 */
export interface Subject {
  lessonPath: string;
  /** Which d2 fence in the lesson, and how many it covers (a slideshow run is several). */
  at: number;
  count: number;
}

export function subjectFromUrl(search: string): Subject | null {
  const params = new URLSearchParams(search);
  const lessonPath = params.get("lesson");
  const at = Number(params.get("at"));
  if (lessonPath == null || lessonPath === "" || !Number.isInteger(at) || at < 0) return null;
  const count = Number(params.get("count"));
  return { lessonPath, at, count: Number.isInteger(count) && count > 0 ? count : 1 };
}

/** The draft key. Scoped per diagram, so editing two of them does not have one overwrite the
 *  other's autosave, and neither disturbs the blank scratchpad. */
const draftKeyFor = (subject: Subject | null): string =>
  subject == null ? D2_LAB_DRAFT_KEY : `${D2_LAB_DRAFT_KEY}:${subject.lessonPath}:${subject.at}`;

// ─────────────────────────────────────────────────────────────────────────────
// THE PAGE
// ─────────────────────────────────────────────────────────────────────────────

function D2Lab() {
  // Read once: the page is opened on a diagram or it is not, and navigating changes neither.
  const subject = useMemo(() => subjectFromUrl(window.location.search), []);
  const draftKey = draftKeyFor(subject);
  const [source, setSource] = useState<string>(() =>
    // A blank page starts on its draft; a page opened on a diagram starts EMPTY and fills from
    // the lesson, so a stale draft cannot masquerade as what is actually published there.
    subject == null ? (storageGet(draftKey) ?? STARTER) : "",
  );
  /** What the lesson holds right now — the thing an update is checked against. */
  const [published, setPublished] = useState<string | null>(null);
  /** The info string the opened fence carried. A plain ```d2 fence must go back as a plain one:
   *  writing `boards` onto it would convert a simple diagram into a one-board walkthrough, move
   *  its artifact out of the shared pool into a `_d2/` sidecar, and change how it renders — none
   *  of which anyone asked for by clicking Edit. */
  const [openedMeta, setOpenedMeta] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [name, setName] = useState("walkthrough");
  const [rootTitle, setRootTitle] = useState("Context");
  const [leftPct, setLeftPct] = useState(() => clampPct(Number(storageGet(D2_LAB_PANE_KEY))));
  const [toast, setToast] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const editorHost = useRef<HTMLDivElement>(null);
  const editor = useRef<EditorHandle | null>(null);
  const panes = useRef<HTMLDivElement>(null);
  const titleInput = useRef<HTMLInputElement>(null);
  const dragging = useRef(false);
  /** The buffer as of now, for the mount below: it runs once and would otherwise capture the
   *  empty string a page opened on a diagram starts with. */
  const sourceRef = useRef(source);
  sourceRef.current = source;

  // A draft is a walkthrough; an edit is whatever kind it opened as.
  const isBoards = subject == null || (openedMeta != null && isBoardsFence(openedMeta));
  const meta = isBoards ? `boards name="${name}" root="${rootTitle}"` : "";
  // A plain figure has no name of its own — it is the Nth diagram in its lesson, and calling its
  // buffer `walkthrough.d2` would name it after a thing it is not.
  const fileName = isBoards ? `${name}.d2` : `diagram-${(subject?.at ?? 0) + 1}.d2`;
  const fence = `\`\`\`d2${meta === "" ? "" : ` ${meta}`}\n${source.replace(/\n$/, "")}\n\`\`\`\n`;

  // Monaco mounts ONCE and pushes changes out; re-rendering it per keystroke would fight the
  // editor for the buffer it owns.
  useEffect(() => {
    const node = editorHost.current;
    if (node == null) return;
    let disposed = false;
    void mountD2Editor(node, sourceRef.current, storageGet(THEME_KEY) === "dark", setSource).then(
      (mounted) => {
        if (disposed) mounted.dispose();
        else editor.current = mounted;
      },
      (error: unknown) => log.warn(`d2 lab: the editor failed to mount — ${String(error)}`),
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
        const found = d2Fences(payload.raw).slice(subject.at, subject.at + subject.count);
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
        const meta = found[0]!.meta;
        setOpenedMeta(meta);
        setName(fenceName(meta) ?? "walkthrough");
        setRootTitle(rootTitleOf(meta) ?? "Context");
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

  const say = useCallback((message: string) => {
    setToast(message);
    setTimeout(() => setToast(null), 2400);
  }, []);

  /** Re-indent by block depth — two spaces a level, the house style. */
  const tidy = () => {
    let depth = 0;
    const tidied = source
      .split("\n")
      .map((line) => {
        const text = line.trim();
        const opens = (text.match(/\{/g) ?? []).length;
        const closes = (text.match(/\}/g) ?? []).length;
        if (closes > opens) depth = Math.max(0, depth - (closes - opens));
        const out = text === "" ? "" : "  ".repeat(depth) + text;
        if (opens > closes) depth += opens - closes;
        return out;
      })
      .join("\n");
    if (tidied === source) return;
    // Through the editor, not just state: it owns the buffer, and this keeps one undo step.
    editor.current?.setValue(tidied);
    setSource(tidied);
  };

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
      storageSet(D2_LAB_PANE_KEY, (parseFloat(left?.style.width ?? "") || DEFAULT_LEFT_PCT).toFixed(2));
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, []);

  return (
    <div class="d2lab">
      <header class="lab-doc">
        <div class="lab-doc__id">
          <span class="pane-hd__eyebrow">
            {subject == null ? "D2 walkthrough · draft" : `Editing · ${subject.lessonPath}`}
          </span>
          {/* The title is the document's, so it is edited in place rather than in a labelled
              field. The pencil is what says so — without it, an input styled as a heading reads
              as a heading. */}
          {isBoards ? (
            <span class="lab-title-wrap">
              <input
                ref={titleInput}
                class="lab-title"
                value={rootTitle}
                size={Math.max(6, rootTitle.length)}
                onInput={(event) => setRootTitle((event.currentTarget as HTMLInputElement).value)}
                aria-label="The root board's title"
              />
              <button
                class="lab-edit"
                aria-label="Rename the root board"
                title="Rename"
                onClick={() => titleInput.current?.focus()}
              >
                <Icon name="pencil" size={15} />
              </button>
            </span>
          ) : (
            // A plain fence has no title to edit — nothing in `x -> y` names the picture. Its
            // identity is its position in the lesson, which is not ours to rename.
            <span class="lab-title-wrap">
              <span class="lab-title lab-title--fixed">{`Diagram ${(subject?.at ?? 0) + 1}`}</span>
            </span>
          )}
          <div class="lab-meta">
            {isBoards ? (
              <span class="lab-meta__path">
                _d2/
                <input
                  value={name}
                  size={Math.max(4, name.length)}
                  onInput={(event) => setName((event.currentTarget as HTMLInputElement).value)}
                  aria-label="The walkthrough's name — its sidecar directory"
                />
                {/* Before the closing slash: the pencil marks the NAME as editable, and putting
                    it after the separator would attach it to the path as a whole. */}
                <Icon name="pencil" size={11} />
                /
              </span>
            ) : (
              <span class="lab-meta__path">plain d2 figure</span>
            )}
            <span class="lab-meta__dot">·</span>
            <span>{`${source.split("\n").length} lines`}</span>
            <span class="lab-meta__dot">·</span>
            <span class="lab-meta__save">autosaved</span>
            {loadError != null && (
              <>
                <span class="lab-meta__dot">·</span>
                <span class="lab-meta__bad">{loadError}</span>
              </>
            )}
          </div>
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
                download(new Blob([source], { type: "text/plain" }), fileName);
                say(`Downloaded ${fileName}`);
              }}
            >
              <Icon name="download" size={15} />
              Download .d2
            </button>
            {isBoards && (
              <button onClick={() => void exportSidecar(source, meta, say)}>
                <Icon name="package" size={15} />
                Export _d2/
              </button>
            )}
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
            <span class="pane-hd__file">{fileName}</span>
            <span class="pane-hd__sp"></span>
            <button class="pane-hd__btn" title="Tidy indentation" aria-label="Tidy indentation" onClick={tidy}>
              <Icon name="tidy" size={15} />
            </button>
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
        <section class="lab-pane lab-pane--r">
          <Preview source={source} meta={meta} onGoToLine={(line) => editor.current?.goToLine(line)} />
        </section>
      </div>

      {toast != null && (
        <div class="lab-toast">
          <Icon name="check" size={15} />
          {toast}
        </div>
      )}
      {adding && (
        <AddToLesson
          fence={fence}
          sidecar={isBoards ? `_d2/${name}/` : null}
          subject={subject}
          published={published}
          onClose={() => setAdding(false)}
        />
      )}
    </div>
  );
}

/** The `_d2/<name>/` directory a content repo's CI would commit, for a repo with no workflow yet. */
async function exportSidecar(source: string, meta: string, say: (m: string) => void): Promise<void> {
  say("Drawing every board…");
  try {
    const provider = await engineProvider(source, meta);
    const entries = await Promise.all(
      provider.manifest.boards.map(async (board) => ({
        name: `${board.slug}.svg`,
        content: await provider.svgFor(board.id),
      })),
    );
    const manifest = { ...provider.manifest, generator: 1, source: fnv1a(source) };
    entries.unshift({ name: "boards.json", content: `${JSON.stringify(manifest, null, 2)}\n` });
    download(zip(entries), `${boardsDirName(source, meta)}.zip`);
    say(`Exported ${entries.length} file(s).`);
  } catch {
    say("Nothing to export — the diagram does not compile yet.");
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE PREVIEW
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Compiles the debounced source and hands the boards to the reader's own walk.
 *
 * Two rules make it live rather than flickery. A generation counter drops the result of any
 * compile the author has already typed past — the engine serialises, so several can be in flight.
 * And a failed compile HOLDS the last good preview instead of blanking it: mid-edit, a diagram is
 * invalid far more often than it is finished, and a viewer that empties on every keystroke is
 * useless.
 */
function Preview({
  source,
  meta,
  onGoToLine,
}: {
  source: string;
  meta: string;
  onGoToLine: (line: number) => void;
}) {
  const [provider, setProvider] = useState<BoardProvider | null>(null);
  const [problem, setProblem] = useState<D2Problem | null>(null);
  const [busy, setBusy] = useState(false);
  const [zoom, setZoom] = useState(1);
  const [enlarged, setEnlarged] = useState(false);
  const generation = useRef(0);

  useEffect(() => {
    const mine = (generation.current += 1);
    const timer = setTimeout(() => {
      setBusy(true);
      void engineProvider(source, meta).then(
        (next) => {
          if (mine !== generation.current) return; // the author has typed past this one
          setProvider(next);
          setProblem(null);
          setBusy(false);
        },
        (error: unknown) => {
          if (mine !== generation.current) return;
          setProblem(firstProblem(error));
          setBusy(false);
        },
      );
    }, COMPILE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [source, meta]);

  return (
    <>
      <div class="pane-hd">
        <span class="pane-hd__eyebrow">Preview</span>
        <StatusPill busy={busy} problem={problem} />
        <span class="pane-hd__sp"></span>
        <div class="zoom">
          <button
            class="zoom__b"
            aria-label="Zoom out"
            onClick={() => setZoom((z) => Math.max(0.5, Number((z - 0.1).toFixed(2))))}
          >
            <Icon name="minus" size={14} />
          </button>
          <button class="zoom__lvl" title="Reset zoom" onClick={() => setZoom(1)}>
            {`${Math.round(zoom * 100)}%`}
          </button>
          <button
            class="zoom__b"
            aria-label="Zoom in"
            onClick={() => setZoom((z) => Math.min(2, Number((z + 0.1).toFixed(2))))}
          >
            <Icon name="plus" size={14} />
          </button>
        </div>
        <button
          class="pane-hd__btn"
          aria-label="Enlarge diagram"
          title="Enlarge"
          disabled={provider == null}
          onClick={() => setEnlarged(true)}
        >
          <Icon name="expand" size={15} />
        </button>
      </div>
      {provider == null ? (
        <div class="pv">
          <p class="pv__empty">
            {problem == null ? "Drawing the first board…" : "Nothing drawn yet."}
          </p>
        </div>
      ) : (
        <Canvas
          provider={provider}
          zoom={zoom}
          enlarged={enlarged}
          onClose={() => setEnlarged(false)}
        />
      )}
      {problem != null && (
        <div class="ed-err">
          <Icon name="alert" size={15} />
          <span class="ed-err__msg">{problem.message}</span>
          <span class="pane-hd__sp"></span>
          {problem.line != null && (
            <button class="ed-err__go" onClick={() => onGoToLine(problem.line!)}>
              {`Go to line ${problem.line}`}
            </button>
          )}
        </div>
      )}
    </>
  );
}

function StatusPill({ busy, problem }: { busy: boolean; problem: D2Problem | null }) {
  const tone = problem != null ? "bad" : busy ? "busy" : "ok";
  const text =
    problem != null
      ? problem.line != null
        ? `Line ${problem.line} · ${problem.message}`
        : problem.message
      : busy
        ? "Drawing…"
        : "Up to date";
  return (
    <span class={`st st--${tone}`} aria-live="polite" title={text}>
      <i class="st__dot"></i>
      {text}
    </span>
  );
}

/** The paper, the figure on it, and the board control floating over the canvas. */
function Canvas({
  provider,
  zoom,
  enlarged,
  onClose,
}: {
  provider: BoardProvider;
  zoom: number;
  enlarged: boolean;
  onClose: () => void;
}) {
  const walk = useBoardWalk({ provider });
  // The walk's own keys work anywhere in the canvas, so an author who clicked a node can keep
  // stepping without hunting for the bar.
  return (
    <div class="pv" tabIndex={0} onKeyDown={walk.onKeyDown}>
      <div class="pv__paper" style={{ transform: `scale(${zoom})` }}>
        <div class="diagram diagram--boards not-prose">
          <div
            class="diagram__figure"
            onClick={walk.onFigureClick as never}
            dangerouslySetInnerHTML={{ __html: walk.svg ?? "" }}
          ></div>
          <p class="diagram__live" aria-live="polite">
            {walk.title}
          </p>
        </div>
      </div>
      <BoardBar walk={walk} />
      {/* The reader's own overlay, on the same walk — so enlarging here is the affordance a
          lesson has, not a second implementation of it. */}
      {enlarged && (
        <ZoomOverlay
          svg={walk.svg ?? undefined}
          chrome={<BoardBar walk={walk} />}
          onFigureClick={walk.onFigureClick}
          label={`Diagram walkthrough — ${walk.title}`}
          onClose={onClose}
        />
      )}
    </div>
  );
}

const root = document.querySelector<HTMLElement>("[data-d2lab-root]");
if (root) {
  render(h(D2Lab, {}), root);
  log.info("d2 lab mounted");
}
