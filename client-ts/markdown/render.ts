// ─────────────────────────────────────────────────────────────────────────────
// MARKDOWN RENDERER — walking-skeleton edition
// Step 02 needs the island CONTRACT (loader → chunked renderer → HTML string),
// not the pipeline: headings, paragraphs, bold, and inline code are enough to
// see real output cross the boundary. The oracle's full render.ts (markdown-it
// pipeline, ~450 lines) is ported verbatim in step 06 behind this same loader.
// ─────────────────────────────────────────────────────────────────────────────

function escapeHtml(s: string): string {
  return s
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function inline(s: string): string {
  return escapeHtml(s)
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/`([^`]+)`/g, "<code>$1</code>");
}

export function render(src: string): string {
  return src
    .split(/\n{2,}/)
    .map((block) => {
      const heading = /^(#{1,6})\s+(.*)$/s.exec(block.trim());
      if (heading) {
        const level = heading[1].length;
        return `<h${level}>${inline(heading[2])}</h${level}>`;
      }
      return `<p>${inline(block.trim().replaceAll("\n", " "))}</p>`;
    })
    .join("\n");
}
