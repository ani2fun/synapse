// ──────────────────────────────────────────────────────────────────
// MERMAID ISLAND
// declarative diagram text → SVG, rendered by mermaid@11
// ──────────────────────────────────────────────────────────────────
// A ```mermaid fence is authored declarative-diagram text; mermaid is a
// self-contained text→SVG renderer, so it's a lazy third-party island
// exactly like Monaco (@editor) — NOT part of our viz engine (ADR-S026).
// `islands/widgets/Diagrams.tsx` and `/mermaid`'s preview both dynamic-import
// this module directly, so the multi-hundred-KB mermaid chunk lands only on
// pages that actually hold a diagram.
//
// Three verbs over one engine: draw to a string, draw into a node, and
// parse without drawing. The editor needs all three — the preview wants the
// string, and the metadata line wants the diagram type, which only a parse
// can tell it.

import type { MermaidConfig } from "mermaid";

type Mermaid = typeof import("mermaid").default;

// Each render() call needs a DOM-unique id (mermaid inserts a temporary
// measuring node under that id); a monotonic counter keeps them distinct
// across every diagram on a page and across theme re-renders.
let idSeq = 0;

/**
 * The module + its global config, set up once.
 *
 * `mermaid.initialize` is GLOBAL config — every call re-parses it and resets the diagram
 * registry — so a latched promise runs it for the first caller and every later one waits on the
 * same one rather than repeating it.
 *
 * Always the light `"default"` theme, independent of the reader's page theme:
 * authored diagrams color nodes with a fixed *light* pastel palette and never set
 * a label text color, so the theme default supplies it — mermaid's `"dark"` theme
 * would paint light text on those light fills and become unreadable. `"default"`
 * text is dark and reads on every fill; the SVG then sits on a light "card"
 * (diagrams.css). `securityLevel: "strict"` is safe here even though the content
 * is first-party — it costs nothing and hardens the island; `fontFamily: "inherit"`
 * keeps diagram labels in the reader's type.
 */
let engine: Promise<Mermaid> | null = null;

function mermaid(): Promise<Mermaid> {
  if (engine == null) {
    engine = import("mermaid").then((module) => {
      const config: MermaidConfig = {
        startOnLoad: false,
        securityLevel: "strict",
        theme: "default",
        fontFamily: "inherit",
        // We render our OWN error card (MermaidCard, with the raw source to fix), so mermaid's
        // "Syntax error in text" bomb graphic is duplicate output — and it is emitted in the
        // wrong place. render() is called without a container, so mermaid appends its working
        // node to document.body; on a parse error it draws the bomb there and then throws
        // BEFORE its own removeTempElements() cleanup, orphaning the node in <body>. Nothing
        // full-page-reloads in a CSR app, so one malformed diagram leaves a bomb pinned to the
        // bottom of every page for the rest of the session. This flag takes mermaid's error
        // branch through the cleanup-then-throw path instead: we still get the rejection that
        // drives the error card, and body is left as it was found.
        suppressErrorRendering: true,
      };
      module.default.initialize(config);
      return module.default;
    });
  }
  return engine;
}

/**
 * Render `src` to an SVG string.
 *
 * Rejects (rather than swallowing) on a malformed diagram, so a caller can show a visible error
 * card with the raw source — never a blank figure. `mermaidErrors.firstProblem` turns the
 * rejection into a sentence and a line.
 */
export async function renderMermaidToSvg(src: string): Promise<string> {
  const engine = await mermaid();
  idSeq += 1;
  const id = `synapse-mermaid-${idSeq}`;
  try {
    const { svg } = await engine.render(id, src);
    return svg;
  } catch (error) {
    // A guard, not the fix — `suppressErrorRendering` above is. mermaid builds its working
    // node as `#d<id>` under <body>; any throw on a path that skips its own cleanup strands
    // that node for the life of the session. We know the id we asked for, so removing it
    // costs nothing and makes the orphan impossible whichever internal path failed.
    document.getElementById(`d${id}`)?.remove();
    throw error;
  }
}

/** Render `src` into `target` as an inline SVG. */
export async function renderMermaidInto(target: HTMLElement, src: string): Promise<void> {
  target.innerHTML = await renderMermaidToSvg(src);
}

/**
 * The diagram's type — `flowchart`, `sequenceDiagram`, `erDiagram` — without drawing it.
 *
 * The editor asks before rendering, which buys two things for one call: the metadata line can
 * name what is being edited, and a syntax error surfaces from the cheap pass rather than from
 * the layout engine. Rejects on malformed source exactly as `renderMermaidToSvg` does.
 */
export async function parseMermaid(src: string): Promise<string> {
  const engine = await mermaid();
  const result = await engine.parse(src);
  // `parse` returns `false` only under `{ suppressErrors: true }`, which we never pass — but the
  // declared type carries that branch, and a diagram type is not worth throwing over.
  return typeof result === "object" ? result.diagramType : "diagram";
}
