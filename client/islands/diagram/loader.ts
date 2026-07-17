// ──────────────────────────────────────────────────────────────────
// DIAGRAM LOADER
// tiny dynamic-import gateway so mermaid + d2 each land in their own chunk
// ──────────────────────────────────────────────────────────────────
// Same trick as @editor/loader and @markdown/loader: the wasm side imports
// THIS module (tiny, eager), and each dynamic import() below makes Vite split
// its renderer (mermaid.ts / d2.ts) into an on-demand chunk, fetched once when
// the first diagram of that kind mounts and cached after. A mermaid-only page
// never pulls the multi-MB d2 WASM, and vice versa.
//
// Oracle deviation, on purpose (same as @markdown): the oracle exports
// loadRenderMermaid() → Promise<fn>; a flat async call is the friendlier
// wasm-bindgen FFI shape, so the load-then-call is folded in here.

import type { renderMermaidInto } from "./mermaid";
import type { renderD2Source } from "./d2";

type RenderMermaidFn = typeof renderMermaidInto;

let cached: Promise<RenderMermaidFn> | null = null;

/** Render `src` into `target` as an inline SVG; mermaid loads lazily on first call. */
export async function renderMermaid(target: HTMLElement, src: string): Promise<void> {
  if (!cached) cached = import("./mermaid").then((m) => m.renderMermaidInto);
  const render = await cached;
  await render(target, src);
}

type RenderD2Fn = typeof renderD2Source;

let cachedD2: Promise<RenderD2Fn> | null = null;

/** Compile + render one d2 fence's source to an SVG string; the d2 WASM loads lazily on first call. */
export async function renderD2(source: string): Promise<string> {
  if (!cachedD2) cachedD2 = import("./d2").then((m) => m.renderD2Source);
  const render = await cachedD2;
  return render(source);
}
