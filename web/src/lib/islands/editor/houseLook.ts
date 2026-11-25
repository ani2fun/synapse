// The house look of every reader-facing code surface: the lesson fence slab (markdown.css), the
// runnable block and its pre-mount preview (runnable.css), and the Monaco editors a lesson opens
// (workbench, Try in Editor, solution viewer). Kept OUT of monaco.ts on purpose: the workbench
// sizes its editor box from these before Monaco's lazy chunk has loaded, and importing them from
// monaco.ts would pull the whole editor into the page bundle.
//
// The CSS halves carry the same two numbers as literals (`font-size: 11px; line-height: 17px`) —
// change them together, or the shiki placeholder and the mounted editor stop lining up.

/** x-small: denser code, so a whole function fits in the card without scrolling. */
export const HOUSE_FONT_SIZE = 11;
/** Line height for HOUSE_FONT_SIZE — the same 1.55 ratio `lineHeightFor` uses for any size. */
export const HOUSE_LINE_HEIGHT = 17;

/** An editor box tall enough for `source`, with a floor and a ceiling (the padding is Monaco's). */
export function editorHeightPx(source: string): number {
  const lines = source.split("\n").length;
  return Math.min(Math.max(lines * HOUSE_LINE_HEIGHT + 28, 64), 520);
}
