// ──────────────────────────────────────────────────────────────────
// FRAME RUNS
// an animation authored as consecutive images → one stepping figure
// ──────────────────────────────────────────────────────────────────
// A content generator emits an animated diagram as a RUN of stills: one markdown
// image per frame, each alt-texted `<caption> — frame i of N`, under a
// `// Interactive Diagram (N frames): <caption>` line. Rendered literally that is
// N near-identical pictures to scroll past and, at ~100 KB a frame, megabytes of
// eager PNG for a figure that shows one still at a time.
//
// This is the pure half of the fix: mdast in, mdast out, no unified. A run
// collapses into ONE `.frame-slideshow` placeholder the client mounts a transport
// over, and the marker line — prose today, and noise — becomes the widget's
// caption. A `// Diagram: <caption>` above a LONE image becomes that image's
// figcaption the same way.
//
// The ALT TEXT is the only grouping signal. Adjacency alone never groups, so two
// unrelated images that happen to sit together stay two images, here and in every
// other book. A marker with no image under it captions nothing and stays the prose
// it is — some carry the only copy of their sentence.
//
// Working on the tree rather than on raw lines is what makes the marker rules safe:
// a `// …` line inside a fence is part of a `code` node's value, never a paragraph,
// so ordinary Java and C comments are untouchable from here.

import type { Image, PhrasingContent, RootContent } from "mdast";

/** Where a still sits in its sequence, and what the sequence is called. */
export type FrameAlt = { caption: string; index: number; total: number };

// The separator is an em dash in every frame alt this catalog ships; the en dash and the plain
// hyphen are accepted too, so a hand-typed run groups instead of quietly falling back to stacked
// images. The caption is greedy — if one ever ends in " — frame 2 of 3", the LAST separator is
// the real one.
const FRAME_ALT = /^(.+)\s+[—–-]\s+frame\s+(\d+)\s+of\s+(\d+)$/i;

// The two marker vocabularies an author writes above a diagram. Both are plain paragraphs, so
// both render as literal `// …` prose until they are consumed into a caption.
const CAPTION_MARKER =
  /^\/\/\s*(?:interactive\s+diagram\s*\(\s*\d+\s*frames?\s*\)|diagram)\s*:\s*(.+)$/i;

/** `<caption> — frame 4 of 17` → its three parts, or null when the alt is an ordinary one. */
export function parseFrameAlt(alt: string): FrameAlt | null {
  const matched = FRAME_ALT.exec(alt.trim());
  if (matched === null) return null;
  return { caption: matched[1]!.trim(), index: Number(matched[2]), total: Number(matched[3]) };
}

/**
 * A node's visible text with inline formatting flattened. A caption carries escaped brackets
 * (`leftArr\[i\]`), comparison operators and the occasional emphasis, none of which land as one
 * plain text child — so a marker matcher that expected exactly one would miss them.
 */
export function flattenText(node: PhrasingContent | RootContent): string {
  if ("value" in node && typeof node.value === "string") return node.value;
  if ("children" in node) {
    return (node.children as (PhrasingContent | RootContent)[]).map(flattenText).join("");
  }
  return "";
}

/** `// Diagram: …` / `// Interactive Diagram (N frames): …` → the caption it carries. */
export function parseCaptionMarker(node: RootContent): string | null {
  if (node.type !== "paragraph") return null;
  const matched = CAPTION_MARKER.exec(flattenText(node).trim());
  return matched === null ? null : matched[1]!.trim();
}

/**
 * The lone image of an image-only paragraph. A paragraph holding TWO images — the shape two
 * frames written on consecutive lines take — is deliberately not one, so such a pair breaks the
 * run instead of half-joining it.
 */
function loneImage(node: RootContent | undefined): Image | null {
  if (node === undefined || node.type !== "paragraph") return null;
  const meat = node.children.filter((child) => child.type !== "text" || child.value.trim() !== "");
  if (meat.length !== 1) return null;
  const only = meat[0]!;
  return only.type === "image" && only.url !== "" ? only : null;
}

/**
 * The maximal run starting at `start`: images whose alts share a caption and a total and count
 * up without a gap. A caption change, a repeat, or a skip ends the run there — the next image
 * starts a fresh one, so no frame is ever dropped. An image that is not a frame at all still
 * comes back as a run of one, because a captioned lone diagram renders too.
 */
function runAt(kids: RootContent[], start: number): Image[] | null {
  const first = loneImage(kids[start]);
  if (first === null) return null;
  const head = parseFrameAlt(first.alt ?? "");
  const run = [first];
  if (head === null) return run;
  for (let at = start + 1; at < kids.length; at += 1) {
    const image = loneImage(kids[at]);
    const frame = image === null ? null : parseFrameAlt(image.alt ?? "");
    if (
      frame === null ||
      frame.caption !== head.caption ||
      frame.total !== head.total ||
      frame.index !== head.index + (at - start)
    ) {
      break;
    }
    run.push(image!);
  }
  return run;
}

/** A raw-HTML mdast node — passes through remark-rehype under allowDangerousHtml. */
function html(value: string): RootContent {
  return { type: "html", value } as RootContent;
}

const ESCAPES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" };

/** Captions and alts reach the page as raw HTML, and they provably carry `<`, `>` and `"`. */
function escapeHtml(value: string): string {
  return value.replace(/[&<>"]/g, (char) => ESCAPES[char]!);
}

/**
 * A run's placeholder. It carries the frame URLs and the caption, not the per-frame alts: every
 * alt is exactly `<caption> — frame i of N`, so the client rebuilds them and the payload stays
 * one shared string instead of N copies of it.
 */
function slideshow(run: Image[], marker: string | null): RootContent {
  const caption = marker ?? parseFrameAlt(run[0]!.alt ?? "")?.caption ?? "";
  const frames = JSON.stringify(run.map((image) => image.url));
  return html(
    `<div class="frame-slideshow" data-frames="${encodeURIComponent(frames)}"` +
      ` data-caption="${encodeURIComponent(caption)}"></div>`,
  );
}

/**
 * A captioned lone diagram — the `// Diagram: …` case. No placeholder and no island: there is
 * nothing to step through, so the image renders directly with its caption under it. `lazy` is
 * the whole reason this is worth rewriting at all: these stills run ~100 KB apiece.
 */
function figure(image: Image, caption: string): RootContent {
  return html(
    `<figure class="prose-figure"><img src="${escapeHtml(image.url)}"` +
      ` alt="${escapeHtml(image.alt ?? "")}" loading="lazy" decoding="async">` +
      `<figcaption>${escapeHtml(caption)}</figcaption></figure>`,
  );
}

/**
 * Rewrite a document's top-level children, collapsing frame runs and consuming the marker lines
 * that caption them. Returns null when the document holds no image-only paragraph at all, so a
 * lesson without diagrams pays one cheap scan and nothing is reallocated.
 */
export function groupFrameRuns(kids: RootContent[]): RootContent[] | null {
  if (!kids.some((node) => loneImage(node) !== null)) return null;

  const out: RootContent[] = [];
  let index = 0;
  while (index < kids.length) {
    const caption = parseCaptionMarker(kids[index]!);
    const start = caption === null ? index : index + 1;
    const run = runAt(kids, start);

    if (run === null) {
      out.push(kids[index]!); // including a marker with nothing under it: it is still prose
      index += 1;
      continue;
    }
    if (run.length > 1) {
      out.push(slideshow(run, caption));
    } else if (caption !== null) {
      out.push(figure(run[0]!, caption));
    } else {
      out.push(kids[start]!); // a lone uncaptioned image is an ordinary image
    }
    index = start + run.length;
  }
  return out;
}
