// ─────────────────────────────────────────────────────────────────────────────
// @markdown/loader — THE ISLAND BOUNDARY
// Statically imported by the wasm-bindgen glue (tiny); the renderer itself is
// dynamic-imported so it lands in its own Vite chunk off the critical path —
// the oracle's `@JSImport("@markdown/loader")` pattern, verbatim.
// ─────────────────────────────────────────────────────────────────────────────

export async function renderMarkdown(src: string): Promise<string> {
  const { render } = await import("./render");
  return render(src);
}
