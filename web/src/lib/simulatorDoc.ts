// Lesson-local simulators (ADR-RS012): where a widget beside a lesson is served from, and, for a
// PRIVATE book, the one self-contained document an iframe can be given instead of a URL.
//
// Pure string work, so it runs (and is tested) without a DOM: an authored widget page is a small,
// regular HTML file, and a private book's widgets are the book's own.

/**
 * `_assets/_simulators/…` → `/content-assets/<slug path>/_assets/_simulators/…`, the slug path
 * read off the reader URL `/synapse/<slug path>`. Null off a reader page: the authoring preview
 * has no lesson URL to resolve against.
 */
export function contentAssetUrl(src: string, pathname: string): string | null {
  if (!pathname.startsWith("/synapse/")) return null;
  const slugs = pathname.slice("/synapse/".length).replace(/\/+$/, "");
  return slugs ? `/content-assets/${slugs}/${src}` : null;
}

/** Inline code must not close its own element early. */
const guard = (code: string, tag: "script" | "style"): string =>
  code.replace(new RegExp(`</${tag}`, "gi"), `<\\/${tag}`);

/**
 * The page at `url` as ONE document: each same-origin `<script src>` and stylesheet `<link>` is
 * fetched through `fetchText` and inlined in place (so scripts still run in their order), and
 * `window.synapseSimulator.params` is defined first, from the URL's query — a `srcdoc` has no
 * query string of its own. A widget that loads more files at run time is not supported this way.
 */
export async function selfContained(
  url: string,
  origin: string,
  fetchText: (path: string) => Promise<string>,
): Promise<string> {
  const page = new URL(url, origin);
  let html = await fetchText(page.pathname);
  const local = (ref: string): string | null => {
    const at = new URL(ref, page);
    return at.origin === origin ? at.pathname : null;
  };

  const scripts = [...html.matchAll(/<script\b([^>]*?)\s+src="([^"]+)"([^>]*)>\s*<\/script>/gi)];
  for (const [tag, before, ref, after] of scripts) {
    const path = local(ref!);
    if (path == null) continue;
    const inline = `<script${before}${after}>${guard(await fetchText(path), "script")}</script>`;
    html = html.replace(tag, () => inline); // a function: `$&` in the code must stay literal
  }
  const links = [...html.matchAll(/<link\b[^>]*\brel="stylesheet"[^>]*>/gi)];
  for (const [tag] of links) {
    const ref = /\bhref="([^"]+)"/i.exec(tag)?.[1];
    const path = ref == null ? null : local(ref);
    if (path == null) continue;
    const inline = `<style>${guard(await fetchText(path), "style")}</style>`;
    html = html.replace(tag, () => inline);
  }

  const boot = `<script>window.synapseSimulator = { params: new URLSearchParams(${JSON.stringify(page.search)}) };</script>`;
  // First thing in <head>, or right after the doctype when the page has no <head> tag: before
  // the doctype it would drop the frame into quirks mode.
  if (/<head[^>]*>/i.test(html)) return html.replace(/<head[^>]*>/i, (head) => head + boot);
  return html.replace(/^(\s*<!doctype[^>]*>)?/i, (doctype) => doctype + boot);
}
