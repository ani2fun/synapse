// The Notes preview's markdown → HTML. Deliberately NOT the lesson pipeline (`renderLesson`):
// that one trusts its input — raw HTML passes through, and a `run` or `viz` fence becomes a live
// widget — because lessons are first-party content. A note is whatever the reader typed, and it is
// injected with `innerHTML`, so this pipeline takes the GFM core and nothing else:
//
//   · raw HTML is DROPPED (remark-rehype without `allowDangerousHtml`), so `<img onerror>` in a
//     note renders as nothing rather than running;
//   · every `href` and `src` is checked against a scheme allowlist, because markdown has its own
//     way in — `[x](javascript:…)` — that dropping raw HTML does not close;
//   · fences stay plain `<pre><code>`: no shiki, no workbench, no diagram.
//
// Loaded on demand (the pane imports it the first time Preview opens), so a reader who never
// previews pays nothing for unified.

import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import type { Element, Root, RootContent } from "hast";

/** Relative links, fragments and these schemes survive; anything else is removed. */
const SAFE_URL = /^(?:https?:|mailto:|\/|#|\.{0,2}\/)/i;

function isSafeUrl(value: unknown): boolean {
  if (typeof value !== "string") return false;
  // Browsers ignore whitespace and control characters inside a scheme, so `java\tscript:` is a
  // live `javascript:` URL. Strip them before judging, the way the browser will.
  const url = value.replace(/[\u0000- ]/g, "");
  return SAFE_URL.test(url);
}

function scrub(node: Root | RootContent): void {
  if (node.type === "element") {
    const element = node as Element;
    for (const attr of ["href", "src"] as const) {
      if (attr in element.properties && !isSafeUrl(element.properties[attr])) {
        delete element.properties[attr];
      }
    }
    // A note's links leave the page, and a problem page is not something to navigate away from
    // mid-thought — or to hand `window.opener` to.
    if (element.tagName === "a" && typeof element.properties.href === "string") {
      element.properties.target = "_blank";
      element.properties.rel = ["noopener", "noreferrer"];
    }
  }
  if ("children" in node) for (const child of node.children) scrub(child);
}

function rehypeScrubUrls() {
  return (tree: Root) => scrub(tree);
}

export async function renderNotes(markdown: string): Promise<string> {
  const file = await unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(remarkRehype)
    .use(rehypeScrubUrls)
    .use(rehypeStringify)
    .process(markdown);
  return String(file);
}
