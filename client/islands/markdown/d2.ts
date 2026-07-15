// ──────────────────────────────────────────────────────────────────
// D2 RENDER (parse-time, WASM)
// ```d2 fence text → SVG string, via @terrastruct/d2
// ──────────────────────────────────────────────────────────────────
// d2, like mermaid, is a self-contained declarative-diagram renderer, so
// per ADR-S026 it's orthogonal to the Laminar viz engine. Unlike mermaid
// (rendered client-side at mount), d2 is rendered **at markdown-parse time**
// inside render.ts's pipeline: the multi-MB d2 WASM is dynamic-imported here
// only when a lesson actually contains a ```d2 fence, so diagram-free pages
// never pay for it.

/**
 * Compile + render one d2 diagram to an SVG string.
 *
 * Always rendered with the light neutral theme (themeID 0), independent of the
 * reader's page theme. Authored diagrams color nodes with a fixed *light* pastel
 * palette (the 5-role house convention) and never set a label text color, so the
 * theme default supplies it — a dark theme would paint light text on those light
 * fills and become unreadable. Light-theme text is dark and reads on every fill;
 * the SVG then sits on a light "card" (diagrams.css) so it looks intentional on a
 * dark page. `salt` makes the SVG's internal element ids unique so several d2
 * diagrams can coexist in one document without id collisions. Rejects on a
 * malformed diagram so the pipeline can show a visible `.diagram-error` card and
 * keep the raw fence.
 */
export async function renderD2(source: string, salt: string): Promise<string> {
  const { D2 } = await import("@terrastruct/d2");
  const d2 = new D2();
  const result = await d2.compile(source, { layout: "dagre" });
  return d2.render(result.diagram, {
    themeID: 0, // neutral default — dark text, reads on the authored light fills
    pad: 20,
    noXMLTag: true, // embedding into HTML, not writing a file
    salt,
  });
}
