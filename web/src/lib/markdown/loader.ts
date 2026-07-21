// ──────────────────────────────────────────────────────────────────
// MARKDOWN LOADER
// tiny dynamic-import gateway so the pipeline lands in its own chunk
// ──────────────────────────────────────────────────────────────────
// render.ts pulls in unified + the remark/rehype graph + shiki (grammars
// + a highlight engine) — heavy, and only the lesson page needs it. The
// wasm client imports THIS module (tiny, safe to keep eager) via its
// @markdown extern; the dynamic import() below makes Vite split render.ts
// into an on-demand chunk, fetched once on the first lesson. (ADR-S015.)
//
// Oracle deviation, on purpose: the oracle exports loadRenderLesson()
// (a Promise of the render fn) and Scala invokes the fn; a plain
// (src) => Promise<string> is the friendlier wasm-bindgen FFI shape, so
// the call is folded in here. Same chunking, same caching semantics.

type RenderLessonFn = (raw: string) => Promise<string>;

let cached: Promise<RenderLessonFn> | null = null;

/** Render one lesson's markdown to HTML, lazily loading the pipeline on first call. */
export async function renderMarkdown(src: string): Promise<string> {
  if (!cached) cached = import("./render").then((m) => m.renderLesson);
  return (await cached)(src);
}

type HighlightFn = (code: string, lang: string) => Promise<string>;

let cachedHighlight: Promise<HighlightFn> | null = null;

/** Highlight one snippet with the pipeline's shiki theme (lazy-workbench placeholders). */
export async function highlightCode(code: string, lang: string): Promise<string> {
  if (!cachedHighlight) cachedHighlight = import("./render").then((m) => m.highlightCode);
  return (await cachedHighlight)(code, lang);
}
