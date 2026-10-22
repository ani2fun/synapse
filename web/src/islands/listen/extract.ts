// ──────────────────────────────────────────────────────────────────
// WHAT A LESSON SOUNDS LIKE — the prose, block by block
// ──────────────────────────────────────────────────────────────────
// The lesson body is prose interleaved with things that cannot be read aloud: code, diagrams,
// runnable workbenches, quizzes, widgets, maths. This walks the body's TEXT NODES rather than its
// elements, files each under the nearest block that owns it (a paragraph, a list item, a
// heading), and drops any that sit inside something on the skip list. Walking text nodes is what
// makes nesting free: a nested list item's words belong to the inner item, never to its parent.
//
// Selection and "from here" are one operation. Both are a DOM Range; "from here" is simply the
// range from a block to the end of the body.

/** Elements that own a run of prose. Tables are deliberately absent — see `SKIP`. */
const BLOCK = "h1, h2, h3, h4, h5, h6, p, li, blockquote, dt, dd, figcaption";

/**
 * Everything whose text must never be read. A new kind of embedded block is one line here.
 * Tables are skipped: in these lessons they are mostly query results, and a grid read aloud cell
 * by cell is noise rather than prose.
 */
export const SKIP = [
  "pre",
  "table",
  ".workbench",
  ".fence-group",
  ".d2-block",
  ".d2-slideshow",
  ".d2-boards",
  ".diagram",
  ".mermaid-block",
  ".viz-widget",
  ".simulator-block",
  ".quiz-block",
  ".practice-problem",
  ".solution-block",
  ".katex",
  "[aria-hidden='true']",
  "script",
  "style",
].join(", ");

/** End a block with a full stop when it has no punctuation of its own, so the engine pauses
 *  after a heading or a bullet instead of running it into the next sentence. */
function closed(text: string): string {
  return /[.!?:;…]["')\]]?$/.test(text) ? text : `${text}.`;
}

/**
 * The prose of `root` that lies inside `range`, one string per block, in document order. Blank
 * blocks are dropped and whitespace is folded.
 */
export function proseInRange(root: HTMLElement, range: Range): string[] {
  const doc = root.ownerDocument;
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const blocks: string[] = [];
  let owner: Element | null = null;
  let buffer = "";
  const flush = (): void => {
    const text = buffer.replace(/\s+/g, " ").trim();
    if (text) blocks.push(closed(text));
    buffer = "";
  };
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node as Text;
    const parent = text.parentElement;
    if (!parent || parent.closest(SKIP)) continue;
    const block = parent.closest(BLOCK);
    if (!block || !root.contains(block) || !range.intersectsNode(text)) continue;
    let value = text.data;
    // The range may start or end part-way through a text node: keep only the selected part.
    if (text === range.endContainer) value = value.slice(0, range.endOffset);
    if (text === range.startContainer) value = value.slice(range.startOffset);
    if (block !== owner) {
      flush();
      owner = block;
    }
    buffer += value;
  }
  flush();
  return blocks;
}

/** The whole body from `start` to its end — "listen from here". */
export function proseFrom(root: HTMLElement, start: Element): string[] {
  const range = root.ownerDocument.createRange();
  range.setStartBefore(start);
  range.setEnd(root, root.childNodes.length);
  return proseInRange(root, range);
}

/** The block a "from here" starts at: the one holding the selection's start, or `null` when the
 *  selection is not in the prose at all. */
export function blockAt(root: HTMLElement, node: Node): Element | null {
  const element = node instanceof Element ? node : node.parentElement;
  const block = element?.closest(BLOCK) ?? null;
  return block && root.contains(block) && !block.closest(SKIP) ? block : null;
}
