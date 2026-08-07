// Helpers shared by the markdown pipeline's specs. Not a `*.test.ts`, so vitest does not collect
// it, and nothing in the app imports it.

/**
 * Pull a data attribute back out of a rendered placeholder, the way the client does:
 * `getAttribute()` — which decodes HTML entities, and node here has no DOM, so mirror it —
 * followed by `decodeURIComponent`.
 */
export function decodeAttr(html: string, name: string): string | undefined {
  const encoded = html.match(new RegExp(`${name}="([^"]*)"`))?.[1];
  if (encoded === undefined) return undefined;
  const attr = encoded
    .replace(/&#x27;/g, "'")
    .replace(/&#39;/g, "'")
    .replace(/&quot;/g, '"')
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&"); // last — un-escapes any remaining &-sequences without double-decoding
  return decodeURIComponent(attr);
}
