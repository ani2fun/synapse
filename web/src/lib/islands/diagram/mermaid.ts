// ──────────────────────────────────────────────────────────────────
// MERMAID ISLAND
// declarative diagram text → SVG, rendered by mermaid@11
// ──────────────────────────────────────────────────────────────────
// A ```mermaid fence is authored declarative-diagram text; mermaid is a
// self-contained text→SVG renderer, so it's a lazy third-party island
// exactly like Monaco (@editor) — NOT part of our viz engine (ADR-S026).
// Scala reaches it through loader.ts; the multi-hundred-KB mermaid chunk
// therefore lands only on lessons that actually contain a diagram.

import type { MermaidConfig } from "mermaid";

// Each render() call needs a DOM-unique id (mermaid inserts a temporary
// measuring node under that id); a monotonic counter keeps them distinct
// across every diagram on a page and across theme re-renders.
let idSeq = 0;

/**
 * Render `src` into `target` as an inline SVG.
 *
 * Always the light `"default"` theme, independent of the reader's page theme:
 * authored diagrams color nodes with a fixed *light* pastel palette and never set
 * a label text color, so the theme default supplies it — mermaid's `"dark"` theme
 * would paint light text on those light fills and become unreadable. `"default"`
 * text is dark and reads on every fill; the SVG then sits on a light "card"
 * (diagrams.css). `securityLevel: "strict"` is safe here even though the content
 * is first-party — it costs nothing and hardens the island; `fontFamily: "inherit"`
 * keeps diagram labels in the reader's type.
 *
 * Rejects (rather than swallowing) on a malformed diagram so MermaidView
 * can show a visible error card with the raw source — never a blank figure.
 */
// mermaid.initialize is GLOBAL config — run it once per session, not per diagram
// (each call re-parses the config + resets the registry). A module-level latch keeps
// the 2nd..Nth render from repeating it.
let initialized = false;

export async function renderMermaidInto(target: HTMLElement, src: string): Promise<void> {
  const mermaid = (await import("mermaid")).default;
  if (!initialized) {
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
    mermaid.initialize(config);
    initialized = true;
  }
  idSeq += 1;
  const id = `synapse-mermaid-${idSeq}`;
  try {
    const { svg } = await mermaid.render(id, src);
    target.innerHTML = svg;
  } catch (error) {
    // A guard, not the fix — `suppressErrorRendering` above is. mermaid builds its working
    // node as `#d<id>` under <body>; any throw on a path that skips its own cleanup strands
    // that node for the life of the session. We know the id we asked for, so removing it
    // costs nothing and makes the orphan impossible whichever internal path failed.
    document.getElementById(`d${id}`)?.remove();
    throw error;
  }
}
