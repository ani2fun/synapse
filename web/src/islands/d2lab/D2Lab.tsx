/**
 * `/d2` — the d2 half of the diagram editor. The shell (`islands/diagramlab/Lab`) owns the buffer,
 * the split and the route to a pull request; this owns what makes a d2 document a d2 document.
 *
 * Three things do. A walkthrough has a NAME and a root board title, so its identity is an editable
 * heading rather than a labelled field, with the sidecar path as one quiet line of metadata under
 * it. Its figures are a tree rather than a picture, so the preview mounts the reader's own board
 * walk and clicking a node drills down exactly as it will in a lesson. And its artifacts are files
 * a content repo's CI commits, so an author on a repo with no workflow yet can export them here.
 */
import { h, render } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { Icon } from "../diagramlab/icons";
import { type LabContext, type LabDoc, Lab, mountLab } from "../diagramlab/Lab";
import { PreviewFrame } from "../diagramlab/PreviewFrame";
import { type BoardProvider, BoardBar, engineProvider, useBoardWalk } from "../widgets/D2Boards";
import { type D2Problem, firstProblem } from "../../lib/islands/diagram/d2Errors";
import { download, zip } from "../../lib/zip";
import { boardsDirName, fenceName, isBoardsFence, rootTitleOf } from "../../lib/islands/diagram/boards";
import { fnv1a } from "../../lib/hash";

/** Long enough that a burst of typing is one compile, short enough to feel live. */
const COMPILE_DEBOUNCE_MS = 450;

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

/** Re-indent by block depth — two spaces a level, the house style. */
function tidyD2(source: string): string {
  let depth = 0;
  return source
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
}

// ─────────────────────────────────────────────────────────────────────────────
// THE d2 DOCUMENT
// ─────────────────────────────────────────────────────────────────────────────

function useD2Doc({ source, subject, openedMeta, say, goToLine }: LabContext): LabDoc {
  const [name, setName] = useState("walkthrough");
  const [rootTitle, setRootTitle] = useState("Context");
  const titleInput = useRef<HTMLInputElement>(null);

  // The fence's own vocabulary, read back off the info string it opened with.
  useEffect(() => {
    if (openedMeta == null) return;
    setName(fenceName(openedMeta) ?? "walkthrough");
    setRootTitle(rootTitleOf(openedMeta) ?? "Context");
  }, [openedMeta]);

  // A draft is a walkthrough; an edit is whatever kind it opened as. A plain ```d2 fence must go
  // back as a plain one: writing `boards` onto it would convert a simple diagram into a one-board
  // walkthrough, move its artifact out of the shared pool into a `_d2/` sidecar, and change how it
  // renders — none of which anyone asked for by clicking Edit.
  const isBoards = subject == null || (openedMeta != null && isBoardsFence(openedMeta));
  const meta = isBoards ? `boards name="${name}" root="${rootTitle}"` : "";

  return {
    meta,
    // A plain figure has no name of its own — it is the Nth diagram in its lesson, and calling its
    // buffer `walkthrough.d2` would name it after a thing it is not.
    fileName: isBoards ? `${name}.d2` : `diagram-${(subject?.at ?? 0) + 1}.d2`,
    sidecar: isBoards ? `_d2/${name}/` : null,
    draftLabel: "D2 walkthrough",
    identity: isBoards ? (
      <>
        {/* The title is the document's, so it is edited in place rather than in a labelled
            field. The pencil is what says so — without it, an input styled as a heading reads
            as a heading. */}
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
        <div class="lab-meta">
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
            <Icon name="pencil" size={11} />/
          </span>
          <Meta source={source} />
        </div>
      </>
    ) : (
      <>
        {/* A plain fence has no title to edit — nothing in `x -> y` names the picture. Its
            identity is its position in the lesson, which is not ours to rename. */}
        <span class="lab-title-wrap">
          <span class="lab-title lab-title--fixed">{`Diagram ${(subject?.at ?? 0) + 1}`}</span>
        </span>
        <div class="lab-meta">
          <span class="lab-meta__path">plain d2 figure</span>
          <Meta source={source} />
        </div>
      </>
    ),
    actions: isBoards ? (
      <button onClick={() => void exportSidecar(source, meta, say)}>
        <Icon name="package" size={15} />
        Export _d2/
      </button>
    ) : undefined,
    preview: <Preview source={source} meta={meta} onGoToLine={goToLine} />,
  };
}

/** The line count and the save state — the same tail on both identity shapes. */
function Meta({ source }: { source: string }) {
  return (
    <>
      <span class="lab-meta__dot">·</span>
      <span>{`${source.split("\n").length} lines`}</span>
      <span class="lab-meta__dot">·</span>
      <span class="lab-meta__save">autosaved</span>
    </>
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

  if (provider == null) {
    return (
      <PreviewFrame
        busy={busy}
        problem={problem}
        ready={false}
        label="Diagram walkthrough"
        onGoToLine={onGoToLine}
      />
    );
  }
  return <Canvas provider={provider} busy={busy} problem={problem} onGoToLine={onGoToLine} />;
}

/** The paper, the figure on it, and the board control floating over the canvas. */
function Canvas({
  provider,
  busy,
  problem,
  onGoToLine,
}: {
  provider: BoardProvider;
  busy: boolean;
  problem: D2Problem | null;
  onGoToLine: (line: number) => void;
}) {
  const walk = useBoardWalk({ provider });
  return (
    <PreviewFrame
      busy={busy}
      problem={problem}
      ready
      label={`Diagram walkthrough — ${walk.title}`}
      overlaySvg={walk.svg}
      overlayChrome={<BoardBar walk={walk} />}
      onFigureClick={walk.onFigureClick}
      onKeyDown={walk.onKeyDown}
      onGoToLine={onGoToLine}
    >
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
    </PreviewFrame>
  );
}

mountLab(
  (root) => render(h(Lab, { lang: "d2", starter: STARTER, useDoc: useD2Doc, tidy: tidyD2 }), root),
  "d2 lab",
);
