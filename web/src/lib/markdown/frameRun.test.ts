// Spec for the frame-run pre-pass. Pure over mdast — no unified, no DOM. The suite is written
// against the shapes remark actually produces for this catalog's markdown: an image-only
// paragraph per frame, a marker paragraph above the run, and captions carrying escaped brackets
// and comparison operators.
import { describe, expect, it } from "vitest";
import type { Paragraph, RootContent } from "mdast";

import { flattenText, groupFrameRuns, parseCaptionMarker, parseFrameAlt } from "./frameRun";

/** The `paragraph → [image]` node remark emits for a blank-line-separated image. */
function imagePara(alt: string, url: string): RootContent {
  return { type: "paragraph", children: [{ type: "image", url, alt }] } as RootContent;
}

/** The `paragraph → [text]` node remark emits for a marker or any other plain line. */
function textPara(value: string): RootContent {
  return { type: "paragraph", children: [{ type: "text", value }] } as RootContent;
}

function frames(caption: string, total: number, stem: string): RootContent[] {
  return Array.from({ length: total }, (_unused, at) =>
    imagePara(`${caption} — frame ${at + 1} of ${total}`, `${stem}-${at + 1}.png`),
  );
}

/** Pull a data attribute back out of an emitted placeholder, the way the client does. */
function attr(node: RootContent, name: string): string | undefined {
  const raw = (node as { value?: string }).value ?? "";
  const encoded = new RegExp(`${name}="([^"]*)"`).exec(raw)?.[1];
  return encoded === undefined ? undefined : decodeURIComponent(encoded);
}

const CAPTION = "Iterating through the sorted array";

describe("parseFrameAlt", () => {
  it("splits a frame alt into caption, index and total", () => {
    expect(parseFrameAlt(`${CAPTION} — frame 4 of 17`)).toEqual({
      caption: CAPTION,
      index: 4,
      total: 17,
    });
  });

  it("accepts an en dash and a plain hyphen, so a hand-typed run still groups", () => {
    expect(parseFrameAlt("A – frame 1 of 3")?.caption).toBe("A");
    expect(parseFrameAlt("A - frame 1 of 3")?.caption).toBe("A");
  });

  it("binds the LAST separator, so a caption may itself end in one", () => {
    const parsed = parseFrameAlt("Step — frame 2 of 3 — frame 1 of 9");
    expect(parsed).toEqual({ caption: "Step — frame 2 of 3", index: 1, total: 9 });
  });

  it("leaves an ordinary alt alone", () => {
    expect(parseFrameAlt("A picture of an array")).toBeNull();
    expect(parseFrameAlt("Frames of reference of 3")).toBeNull();
  });
});

describe("parseCaptionMarker", () => {
  it("reads both marker vocabularies", () => {
    expect(parseCaptionMarker(textPara(`// Interactive Diagram (10 frames): ${CAPTION}`))).toBe(CAPTION);
    expect(parseCaptionMarker(textPara("// Diagram: An interval on the x-axis"))).toBe(
      "An interval on the x-axis",
    );
  });

  it("ignores a paragraph that is not a marker", () => {
    expect(parseCaptionMarker(textPara("The line cannot sweep if the array is scrambled."))).toBeNull();
    expect(parseCaptionMarker(textPara("// TODO: regenerate these"))).toBeNull();
  });

  it("flattens inline formatting — a caption may carry emphasis or a raw tag", () => {
    const marker: RootContent = {
      type: "paragraph",
      children: [
        { type: "text", value: "// Diagram: compare " },
        { type: "emphasis", children: [{ type: "text", value: "left" }] },
        { type: "text", value: " and right" },
      ],
    } as RootContent;
    expect(parseCaptionMarker(marker)).toBe("compare left and right");
  });

  it("flattens escaped brackets the way remark unescapes them in an alt", () => {
    // `leftArr\[i\]` in the source reaches BOTH the marker text and the image alt as `leftArr[i]`.
    expect(flattenText(textPara("// Diagram: Compare leftArr[i] and rightArr[j]"))).toBe(
      "// Diagram: Compare leftArr[i] and rightArr[j]",
    );
  });
});

describe("groupFrameRuns", () => {
  it("leaves a document with no image untouched", () => {
    expect(groupFrameRuns([textPara("Just prose."), textPara("More prose.")])).toBeNull();
  });

  it("collapses a marked run into one placeholder and consumes the marker", () => {
    const out = groupFrameRuns([
      textPara("Before."),
      textPara(`// Interactive Diagram (3 frames): ${CAPTION}`),
      ...frames(CAPTION, 3, "/media/sweep"),
      textPara("After."),
    ])!;
    expect(out).toHaveLength(3);
    expect(attr(out[1]!, "data-caption")).toBe(CAPTION);
    expect(JSON.parse(attr(out[1]!, "data-frames")!)).toEqual([
      "/media/sweep-1.png",
      "/media/sweep-2.png",
      "/media/sweep-3.png",
    ]);
    expect(JSON.stringify(out)).not.toContain("Interactive Diagram (3 frames)");
  });

  it("groups an unmarked run too — the alt is the signal, not the marker", () => {
    const out = groupFrameRuns(frames(CAPTION, 4, "/media/x"))!;
    expect(out).toHaveLength(1);
    expect(attr(out[0]!, "data-caption")).toBe(CAPTION);
  });

  it("leaves an ordinary image completely alone", () => {
    const only = imagePara("A picture of an array", "/media/array.png");
    expect(groupFrameRuns([only])).toEqual([only]);
  });

  it("captions a LONE image under a `// Diagram:` marker and consumes the marker", () => {
    const out = groupFrameRuns([
      textPara("// Diagram: An interval on the x-axis"),
      imagePara("An interval on the x-axis", "/media/interval.png"),
    ])!;
    expect(out).toHaveLength(1);
    const value = (out[0] as { value: string }).value;
    expect(value).toContain('class="prose-figure"');
    expect(value).toContain("<figcaption>An interval on the x-axis</figcaption>");
    expect(value).toContain('loading="lazy"');
  });

  it("keeps a marker that captions nothing — it is the only copy of its sentence", () => {
    // The real shape: two markers in a row, then prose, no image anywhere near them. Consuming
    // these would delete the sentence outright.
    const first = textPara("// Diagram: |x - target| < |y - target|, or");
    const second = textPara("// Diagram: |x - target| == |y - target| and x < y");
    const out = groupFrameRuns([
      first,
      second,
      textPara("You must use the quickselect algorithm."),
      imagePara("unrelated", "/media/e.png"),
    ])!;
    expect(out[0]).toBe(first);
    expect(out[1]).toBe(second);
  });

  it("escapes a caption carrying markup, since it lands as raw HTML", () => {
    const out = groupFrameRuns([
      textPara('// Diagram: X <= size & "n" of the list'),
      imagePara("bound", "/media/b.png"),
    ])!;
    const value = (out[0] as { value: string }).value;
    expect(value).toContain("X &lt;= size &amp; &quot;n&quot; of the list");
    expect(value).not.toContain('"n" of the list');
  });

  it("splits on a caption change rather than mis-grouping, losing no frame", () => {
    const out = groupFrameRuns([...frames("First", 2, "/a"), ...frames("Second", 2, "/b")])!;
    expect(out).toHaveLength(2);
    expect(attr(out[0]!, "data-caption")).toBe("First");
    expect(attr(out[1]!, "data-caption")).toBe("Second");
  });

  it("breaks the run at an index gap and starts a fresh one", () => {
    const out = groupFrameRuns([
      imagePara(`${CAPTION} — frame 1 of 4`, "/1.png"),
      imagePara(`${CAPTION} — frame 2 of 4`, "/2.png"),
      imagePara(`${CAPTION} — frame 4 of 4`, "/4.png"),
    ])!;
    expect(out).toHaveLength(2);
    expect(JSON.parse(attr(out[0]!, "data-frames")!)).toHaveLength(2);
    // The straggler is a run of one — an ordinary image, not a one-frame widget.
    expect((out[1] as { type: string }).type).toBe("paragraph");
  });

  it("does not group two images that share ONE paragraph (consecutive lines, no blank)", () => {
    const pair: RootContent = {
      type: "paragraph",
      children: [
        { type: "image", url: "/1.png", alt: `${CAPTION} — frame 1 of 2` },
        { type: "text", value: "\n" },
        { type: "image", url: "/2.png", alt: `${CAPTION} — frame 2 of 2` },
      ],
    } as RootContent;
    expect(groupFrameRuns([pair])).toBeNull();
  });

  it("does not treat an image with a link or trailing prose as a frame", () => {
    const withText: RootContent = {
      type: "paragraph",
      children: [
        { type: "image", url: "/1.png", alt: `${CAPTION} — frame 1 of 2` },
        { type: "text", value: " and some words" },
      ],
    } as RootContent;
    const out = groupFrameRuns([withText, imagePara(`${CAPTION} — frame 2 of 2`, "/2.png")])!;
    expect(out).toHaveLength(2);
    expect((out[0] as Paragraph).type).toBe("paragraph");
  });
});
