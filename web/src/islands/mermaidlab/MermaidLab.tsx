/**
 * `/mermaid` — the mermaid half of the diagram editor. The shell (`islands/diagramlab/Lab`) owns
 * the buffer, the split and the route to a pull request; this owns what makes a mermaid document
 * a mermaid document, which is less than d2 needs and deliberately so.
 *
 * A ```mermaid fence carries no info string, so its title is not on the opening backticks the way
 * a d2 walkthrough's name is — it lives INSIDE the source, in the `---` frontmatter block mermaid
 * reads before it lexes anything. The heading edits it in place all the same; it just writes back
 * into the buffer, which is why the frontmatter appears in the editor as you name a diagram.
 *
 * Nothing is drawn ahead of time either, so the preview is the reader's own path — parse, render,
 * inline the SVG — rather than a lookup with a fallback.
 */
import { h, render } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { Icon } from "../diagramlab/icons";
import { type LabContext, type LabDoc, Lab, mountLab } from "../diagramlab/Lab";
import { PreviewFrame } from "../diagramlab/PreviewFrame";
import { type MermaidProblem, firstProblem } from "../../lib/islands/diagram/mermaidErrors";
import { titleOf, withTitle } from "../../lib/islands/diagram/mermaidTitle";
import { tidyMermaid } from "../../lib/islands/diagram/mermaidTidy";
import { parseMermaid, renderMermaidToSvg } from "../../lib/islands/diagram/mermaid";

/** Long enough that a burst of typing is one render, short enough to feel live. */
const RENDER_DEBOUNCE_MS = 450;

const STARTER = `---
title: The request path
---
%% A flowchart: the request path through a cache.
flowchart LR
  reader["Reader"] --> edge{"Cached?"}
  edge -->|hit| cdn[("CDN")]
  edge -->|miss| app["App server"]
  app --> db[("Database")]
  app -.->|warm| cdn
`;

// ─────────────────────────────────────────────────────────────────────────────
// THE MERMAID DOCUMENT
// ─────────────────────────────────────────────────────────────────────────────

function useMermaidDoc({ source, setSource, subject, goToLine }: LabContext): LabDoc {
  const [kind, setKind] = useState<string | null>(null);

  return {
    // No info string: mermaid has no fence vocabulary, and inventing one here would write a word
    // into the lesson that nothing reads back. The title goes in the frontmatter instead.
    meta: "",
    fileName: subject == null ? "diagram.mmd" : `diagram-${subject.at + 1}.mmd`,
    // Nothing is drawn ahead of time, so a mermaid change creates no directory.
    draftLabel: "Mermaid diagram",
    identity: (
      <>
        <Title source={source} setSource={setSource} />
        <div class="lab-meta">
          <span class="lab-meta__path">{kind ?? "mermaid figure"}</span>
          {subject != null && (
            <>
              <span class="lab-meta__dot">·</span>
              <span>{`diagram ${subject.at + 1}`}</span>
            </>
          )}
          <span class="lab-meta__dot">·</span>
          <span>{`${source.split("\n").length} lines`}</span>
          <span class="lab-meta__dot">·</span>
          <span class="lab-meta__save">autosaved</span>
        </div>
      </>
    ),
    preview: <Preview source={source} onKind={setKind} onGoToLine={goToLine} />,
  };
}

/**
 * The diagram's name, edited in place.
 *
 * The value lives in the SOURCE, so the buffer is the truth and this holds a draft only while the
 * author is mid-word. It commits on blur or Enter rather than per keystroke: writing through
 * Monaco on every letter would reset its undo stack once a character and rewrite the document
 * under a caret that may be somewhere else entirely.
 */
function Title({ source, setSource }: { source: string; setSource: (next: string) => void }) {
  const committed = titleOf(source) ?? "";
  const [draft, setDraft] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const shown = draft ?? committed;

  const commit = () => {
    if (draft == null) return; // nothing was typed; a blur must not clear the title
    setDraft(null);
    const next = withTitle(source, draft);
    if (next !== source) setSource(next);
  };

  return (
    // The title is the document's, so it is edited in place rather than in a labelled field. The
    // pencil is what says so — without it, an input styled as a heading reads as a heading.
    <span class="lab-title-wrap">
      <input
        ref={input}
        class="lab-title"
        value={shown}
        size={Math.max(6, shown.length, "Untitled diagram".length)}
        placeholder="Untitled diagram"
        aria-label="The diagram's title"
        onInput={(event) => setDraft((event.currentTarget as HTMLInputElement).value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") (event.currentTarget as HTMLInputElement).blur();
          else if (event.key === "Escape") setDraft(null);
        }}
      />
      <button
        class="lab-edit"
        aria-label="Rename the diagram"
        title="Rename"
        onClick={() => input.current?.focus()}
      >
        <Icon name="pencil" size={15} />
      </button>
    </span>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE PREVIEW
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Parses and draws the debounced source.
 *
 * Two rules make it live rather than flickery, and they are the ones `/d2`'s preview lives by. A
 * generation counter drops the result of any render the author has already typed past. And a
 * failed parse HOLDS the last good figure instead of blanking it: mid-edit, a diagram is invalid
 * far more often than it is finished, and a viewer that empties on every keystroke is useless.
 *
 * The parse is a separate call from the render rather than a cheaper way of finding the same
 * error: it is what names the diagram type for the metadata line, and it fails on a malformed
 * source before the layout engine is asked to do any work.
 */
function Preview({
  source,
  onKind,
  onGoToLine,
}: {
  source: string;
  onKind: (kind: string) => void;
  onGoToLine: (line: number) => void;
}) {
  const [svg, setSvg] = useState<string | null>(null);
  const [problem, setProblem] = useState<MermaidProblem | null>(null);
  const [busy, setBusy] = useState(false);
  const figure = useRef<HTMLDivElement>(null);
  const generation = useRef(0);

  useEffect(() => {
    const mine = (generation.current += 1);
    const timer = setTimeout(() => {
      setBusy(true);
      void (async () => {
        try {
          const kind = await parseMermaid(source);
          const drawn = await renderMermaidToSvg(source);
          if (mine !== generation.current) return; // the author has typed past this one
          onKind(kind);
          setSvg(drawn);
          setProblem(null);
        } catch (error) {
          if (mine !== generation.current) return;
          // The source travels so the reported line can be translated out of the copy mermaid
          // actually parsed — it strips comments and frontmatter first, and the gutter did not.
          setProblem(firstProblem(error, source));
        } finally {
          if (mine === generation.current) setBusy(false);
        }
      })();
    }, RENDER_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [source]);

  // Painted imperatively, not through JSX: the SVG is a string, and letting Preact diff several
  // hundred nodes it did not create costs more than the assignment it would be doing anyway.
  useEffect(() => {
    if (svg != null && figure.current != null) figure.current.innerHTML = svg;
  }, [svg]);

  return (
    <PreviewFrame
      busy={busy}
      problem={problem}
      ready={svg != null}
      label="Mermaid diagram"
      overlaySvg={svg}
      onGoToLine={onGoToLine}
    >
      {/* The reader's own card markup, so the figure sits on the same paper a lesson gives it. */}
      <div class="diagram not-prose">
        <div class="diagram__figure" ref={figure}></div>
      </div>
    </PreviewFrame>
  );
}

mountLab(
  (root) =>
    render(
      h(Lab, { lang: "mermaid", starter: STARTER, useDoc: useMermaidDoc, tidy: tidyMermaid }),
      root,
    ),
  "mermaid lab",
);
