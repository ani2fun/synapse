// ──────────────────────────────────────────────────────────────────
// D2 ISLAND (client-mount render; prose-first refactor 2026-07-17)
// ```d2 fence source → SVG string, via @terrastruct/d2
// ──────────────────────────────────────────────────────────────────
// d2, like mermaid, is a self-contained declarative-diagram renderer
// (ADR-S026, orthogonal to the Laminar viz engine). It USED to render at
// markdown-parse time inside render.ts, which blocked all prose behind N
// sequential WASM layouts. It now renders on the CLIENT at mount, one
// diagram per spawn_local (concurrent) and only when a diagram nears the
// viewport — so the multi-MB d2 WASM loads lazily and prose paints first.

// A FRESH D2 instance per render. A single module-level instance CANNOT serve concurrent
// compiles — several diagrams rendering at once (each its own task) deadlock it (verified:
// 3 concurrent calls hang). The multi-MB WASM is dynamic-imported once (module-cached); only
// the cheap `new D2()` is per-call, exactly as the old parse-time path did it.
let salt = 0;

/**
 * Compile + render one d2 diagram to an SVG string.
 *
 * Always the light neutral theme (themeID 0), independent of the reader's page theme:
 * authored diagrams color nodes with a fixed *light* pastel palette and never set a label
 * text color, so the theme default supplies it — a dark theme would paint light text on
 * light fills and become unreadable. Light-theme text reads on every fill; the SVG sits on
 * a light "card" (diagrams.css). `salt` makes each SVG's internal element ids unique so
 * several diagrams coexist in one document. Rejects on a malformed diagram so the caller can
 * show a visible `.diagram-error` card, never a blank figure.
 */
export async function renderD2Source(source: string): Promise<string> {
  const { D2 } = await import("@terrastruct/d2");
  const d2 = new D2();
  salt += 1;
  const result = await d2.compile(source, { layout: "dagre" });
  return d2.render(result.diagram, {
    themeID: 0, // neutral default — dark text, reads on the authored light fills
    pad: 20,
    noXMLTag: true, // embedding into HTML, not writing a file
    salt: `d2-${salt}`,
  });
}
