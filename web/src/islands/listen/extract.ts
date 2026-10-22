// ──────────────────────────────────────────────────────────────────
// WHAT A LESSON SOUNDS LIKE — the prose, block by block
// ──────────────────────────────────────────────────────────────────
// The lesson body is prose interleaved with things that cannot be read aloud: code, diagrams,
// runnable workbenches, quizzes, widgets, maths. This walks the body's TEXT NODES rather than its
// elements, files each under the nearest block that owns it (a paragraph, a list item, a
// heading), and drops any that sit inside something on the skip list. Walking text nodes is what
// makes nesting free: a nested list item's words belong to the inner item, never to its parent.
//
// Every character of a block's text remembers the text node and offset it came from, so a unit
// of that text can be turned back into a DOM Range — which is how the page highlights the
// sentence being spoken.
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

/** Where one character of a block's text sits in the DOM. */
interface Origin {
  node: Text;
  offset: number;
}

/** One block's speakable text, whitespace-folded, with the DOM origin of every character. */
export interface ProseBlock {
  text: string;
  element: Element;
  /** `origins[i]` is where `text[i]` came from. A closing full stop the block did not have
   *  (see `closed`) has no origin, so this can be one shorter than `text`. */
  origins: Origin[];
}

/** End a block with a full stop when it has no punctuation of its own, so the engine pauses
 *  after a heading or a bullet instead of running it into the next sentence. */
function closed(text: string): string {
  return /[.!?:;…]["')\]]?$/.test(text) ? text : `${text}.`;
}

/** Accumulates one block, folding whitespace the way `replace(/\s+/g, " ").trim()` would. */
class Builder {
  text = "";
  origins: Origin[] = [];

  constructor(readonly element: Element) {}

  add(node: Text, from: number, to: number): void {
    for (let i = from; i < to; i += 1) {
      const ch = node.data[i];
      if (/\s/.test(ch)) {
        if (this.text === "" || this.text.endsWith(" ")) continue;
        this.text += " ";
      } else {
        this.text += ch;
      }
      this.origins.push({ node, offset: i });
    }
  }

  finish(): ProseBlock | null {
    if (this.text.endsWith(" ")) {
      this.text = this.text.slice(0, -1);
      this.origins.pop();
    }
    return this.text ? { text: closed(this.text), element: this.element, origins: this.origins } : null;
  }
}

/**
 * The prose of `root` that lies inside `range`, one block per entry, in document order. Blank
 * blocks are dropped.
 */
export function proseInRange(root: HTMLElement, range: Range): ProseBlock[] {
  const walker = root.ownerDocument.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const blocks: ProseBlock[] = [];
  let current: Builder | null = null;
  const flush = (): void => {
    const block = current?.finish();
    if (block) blocks.push(block);
    current = null;
  };
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node as Text;
    const parent = text.parentElement;
    if (!parent || parent.closest(SKIP)) continue;
    const block = parent.closest(BLOCK);
    if (!block || !root.contains(block) || !range.intersectsNode(text)) continue;
    // The range may start or end part-way through a text node: keep only the selected part.
    const from = text === range.startContainer ? range.startOffset : 0;
    const to = text === range.endContainer ? range.endOffset : text.data.length;
    if (current?.element !== block) {
      flush();
      current = new Builder(block);
    }
    current.add(text, from, to);
  }
  flush();
  return blocks;
}

/** The whole body from `start` to its end — "listen from here". */
export function proseFrom(root: HTMLElement, start: Element): ProseBlock[] {
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

/** The DOM range covering `[start, end)` of a block's text, or `null` when it maps to nothing. */
export function rangeOf(block: ProseBlock, start: number, end: number): Range | null {
  const last = Math.min(end, block.origins.length) - 1;
  if (start > last || start < 0) return null;
  const from = block.origins[start];
  const to = block.origins[last];
  const range = from.node.ownerDocument.createRange();
  range.setStart(from.node, from.offset);
  range.setEnd(to.node, to.offset + 1);
  return range;
}
