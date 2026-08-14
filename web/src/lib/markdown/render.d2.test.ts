// Spec for ```d2 fences, split out of render.test.ts because they are the one fence family whose
// output depends on WHERE the pipeline runs and on what a content repo has already drawn.
//
// The same `renderLesson` serves two callers. Under SSR it looks each diagram up in
// `_media/d2/<hash>.svg` — drawn ahead of time by `dev-tools/render-d2.mjs` in the content repo's
// CI — and inlines what it finds. In the browser (the authoring preview) it cannot, and ships the
// source for the client to compile. Both shapes are pinned here, along with the fallback that
// connects them: a fence nobody has drawn yet renders a perfectly good page the slow way, so only
// an assertion can tell a working lookup from a dead one.
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderLesson } from "./render";
import { fnv1a } from "../hash";
import { decodeAttr } from "./testkit";

// The engine, stubbed as a TRIPWIRE. Nothing in this file should reach it: SSR inlines a
// pre-drawn file and the browser path only plants a placeholder. A non-zero count means
// something started compiling on a path that must not.
const d2Spy = vi.hoisted(() => ({ compileCalls: 0 }));
vi.mock("@terrastruct/d2", () => ({
  D2: class {
    async compile(src: string) {
      d2Spy.compileCalls += 1;
      return { diagram: { src } };
    }
    async render(_d: unknown, opts: { salt: string }) {
      return `<svg data-salt="${opts.salt}"></svg>`;
    }
  },
}));

/** Stand in for the media route. `drawn` lists the hashes a content repo has committed. */
function serveMedia(drawn: (source: string) => boolean = () => true) {
  const seen: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      const hash = String(url).match(/\/media\/d2\/([0-9a-f]{8})\.svg$/)?.[1];
      seen.push(String(url));
      if (hash === undefined || !drawn(hash)) return { ok: false, text: async () => "" };
      // Shaped like the real thing: the salt baked in is the diagram's FIRST-occurrence salt.
      return { ok: true, text: async () => `<svg data-salt="d2-${hash}">figure ${hash}</svg>` };
    }),
  );
  return seen;
}

/** Run `body` with the lookup enabled, then restore the ambient setting. */
async function withPrerender<T>(body: () => Promise<T>): Promise<T> {
  const previous = process.env.SYNAPSE_D2_PRERENDER;
  process.env.SYNAPSE_D2_PRERENDER = "on";
  try {
    return await body();
  } finally {
    if (previous === undefined) delete process.env.SYNAPSE_D2_PRERENDER;
    else process.env.SYNAPSE_D2_PRERENDER = previous;
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("d2 fences → source-carrying placeholders", () => {
  it("a lone ```d2 fence becomes a d2-block carrying the RAW SOURCE", async () => {
    const html = await renderLesson("```d2\nx -> y\n```");
    expect(html).toContain('class="d2-block"');
    expect(decodeAttr(html, "data-source")).toBe("x -> y"); // the raw source, not an SVG
    expect(html).not.toContain("<pre"); // the fence is replaced, not also highlighted
    expect(html).not.toContain("d2-slides");
    expect(d2Spy.compileCalls).toBe(0);
  });

  it("consecutive ```d2 fences group into ONE d2-slideshow carrying each source", async () => {
    const html = await renderLesson("```d2\na -> b\n```\n\n```d2\nc -> d\n```");
    expect(html).toContain('class="d2-slideshow"');
    expect(html).not.toContain("d2-block");
    expect(JSON.parse(decodeAttr(html, "data-slides")!)).toEqual(["a -> b", "c -> d"]);
  });

  it("a paragraph between two d2 fences breaks the group into two d2-blocks", async () => {
    const html = await renderLesson("```d2\na -> b\n```\n\nBetween.\n\n```d2\nc -> d\n```");
    expect(html).not.toContain("d2-slides");
    expect(html.match(/class="d2-block"/g) ?? []).toHaveLength(2);
  });

  it("a ```D2 fence is a d2 fence — the language is matched case-insensitively", async () => {
    const html = await renderLesson("```D2\nx -> y\n```");
    expect(html).toContain('class="d2-block"');
    expect(html).not.toContain("<pre");
  });
});

describe("d2 fences → figures drawn ahead of time", () => {
  it("inlines the committed SVG and drops data-source, so nothing can recompile it", async () => {
    serveMedia();
    const html = await withPrerender(() => renderLesson("```d2\nx -> y\n```"));
    expect(html).toContain('data-prerendered="1"');
    expect(html).toContain('class="diagram__figure"');
    expect(html).toContain("<svg");
    expect(html).not.toContain("data-source"); // the source would be dead weight beside the SVG
    expect(d2Spy.compileCalls).toBe(0); // inlined a file; compiled nothing
  });

  // Each of these uses its OWN diagram: the lookup cache is module-level and outlives a test, so
  // reusing a source would serve a neighbour's answer and assert nothing.
  it("looks a diagram up by CONTENT, so one file serves it wherever it appears", async () => {
    const source = "content -> addressed";
    const urls = serveMedia();
    await withPrerender(() => renderLesson(`\`\`\`d2\n${source}\n\`\`\``));
    await withPrerender(() => renderLesson(`Prose first.\n\n\`\`\`d2\n${source}\n\`\`\``));
    // Same source → same filename, independent of position. The second render is served from
    // cache, so the assertion is on the URL asked for, not on how many times.
    expect(urls[0]).toContain(`/media/d2/${fnv1a(source)}.svg`);
    expect(urls).toHaveLength(1);
  });

  it("re-salts a repeat within one document so element ids stay unique", async () => {
    serveMedia();
    const html = await withPrerender(() =>
      renderLesson("```d2\nx -> y\n```\n\nBetween.\n\n```d2\nx -> y\n```"),
    );
    const salts = [...html.matchAll(/data-salt="([^"]*)"/g)].map((m) => m[1]);
    expect(salts).toHaveLength(2);
    expect(salts[0]).toBe(`d2-${fnv1a("x -> y")}`);
    expect(salts[1]).toBe(`d2-${fnv1a("x -> y")}-2`); // the file's salt, rewritten for the copy
  });

  it("falls back to the source placeholder when nobody has drawn the diagram yet", async () => {
    serveMedia(() => false); // a fence newer than its repo's last CI run
    const html = await withPrerender(() => renderLesson("```d2\nbrand -> new\n```"));
    expect(html).toContain('class="d2-block"');
    expect(html).not.toContain("data-prerendered");
    expect(decodeAttr(html, "data-source")).toBe("brand -> new"); // the client draws it
  });

  it("ignores a media route that answers with something other than an SVG", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, text: async () => "<!doctype html>…" })));
    const html = await withPrerender(() => renderLesson("```d2\nnot -> an-svg\n```"));
    expect(html).not.toContain("data-prerendered");
    expect(html).not.toContain("<!doctype"); // never a figure made of someone else's markup
  });

  it("draws a slideshow's FIRST slide only, and still ships every slide's source", async () => {
    serveMedia();
    const html = await withPrerender(() => renderLesson("```d2\na -> b\n```\n```d2\nc -> d\n```"));
    expect(html).toContain('class="d2-slideshow"');
    expect(html).toContain('data-prerendered="1"');
    // Slide 0 is the one the transport paints at mount; the rest stay source until stepped to.
    expect(html.match(/<svg/g) ?? []).toHaveLength(1);
    expect(html).toContain(fnv1a("a -> b"));
    expect(JSON.parse(decodeAttr(html, "data-slides")!)).toEqual(["a -> b", "c -> d"]);
  });

  it("the kill switch restores the placeholder output exactly", async () => {
    serveMedia();
    const LESSON = "```d2\nx -> y\n```";
    const previous = process.env.SYNAPSE_D2_PRERENDER;
    try {
      process.env.SYNAPSE_D2_PRERENDER = "on";
      const on = await renderLesson(LESSON);
      process.env.SYNAPSE_D2_PRERENDER = "off";
      const off = await renderLesson(LESSON);
      // Both directions, or this passes without the switch doing anything.
      expect(on).not.toBe(off);
      expect(on).toContain("data-prerendered");
      expect(off).toBe(
        `<div class="d2-block" data-fence-at="0" data-source="${encodeURIComponent("x -> y")}"></div>`,
      );
    } finally {
      if (previous === undefined) delete process.env.SYNAPSE_D2_PRERENDER;
      else process.env.SYNAPSE_D2_PRERENDER = previous;
    }
  });
});

// ── WALKTHROUGHS ─────────────────────────────────────────────────────────────────────────────
// A ```d2 boards fence is the one d2 shape whose artifacts are CO-LOCATED with the lesson, so it
// is also the one that needs to know which lesson it is in. Everything here turns on a single
// rule: the ELEMENT is chosen by the fence, and only the FIGURE by whether the lookup hit. Get
// that backwards and every miss — a repo with no CI, the authoring preview, the kill switch —
// silently degrades to a single root board with dead links, which is the bug this replaces.

/** A walkthrough fence. Every case gets its OWN source: the SSR cache is module-level and keyed
 *  by lesson + source, exactly as it is in the running server, so sharing one would make these
 *  order-dependent. */
const walkthroughOf = (source: string) =>
  `\`\`\`d2 boards name="url-shortener" root="Context"\n${source}\n\`\`\``;

/** Stand in for `/api/synapse/d2`. `drawn` decides whether this lesson's sidecar exists. */
function serveBoards(source: string, drawn = true) {
  const seen: string[] = [];
  const manifest = {
    generator: 1,
    source: fnv1a(source),
    root: "root",
    boards: [
      { id: "root", slug: "root", title: "Context", parent: null, links: ["root.layers.a"] },
      { id: "root.layers.a", slug: "a", title: "A", parent: "root", links: [] },
    ],
    warnings: [],
  };
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      seen.push(String(url));
      if (!drawn) return { ok: false, text: async () => "" };
      if (String(url).includes("boards.json")) {
        return { ok: true, text: async () => JSON.stringify(manifest) };
      }
      return { ok: true, text: async () => "<svg>root board</svg>" };
    }),
  );
  return { seen, manifest };
}

const LESSON_CTX = { lessonPath: "learn/dsa/lists/singly" };

describe("```d2 boards → the walkthrough viewer", () => {
  it("emits .d2-boards whether or not anything is drawn", async () => {
    // The assertion that carries the design. Both shapes, stated positively, so neither can pass
    // by the element simply being absent.
    const cold = await renderLesson(walkthroughOf("cold -> board"));
    expect(cold).toContain('class="d2-boards"');
    expect(cold).not.toContain("data-prerendered");
    expect(decodeAttr(cold, "data-source")).toBe("cold -> board");
    expect(decodeAttr(cold, "data-meta")).toBe('boards name="url-shortener" root="Context"');

    serveBoards("warm -> board");
    const warm = await withPrerender(() =>
      renderLesson(walkthroughOf("warm -> board"), LESSON_CTX),
    );
    expect(warm).toContain('class="d2-boards"');
    expect(warm).toContain('data-prerendered="1"');
    expect(warm).toContain("<svg>root board</svg>");
    expect(warm).not.toContain("data-source"); // nothing may recompile a drawn walkthrough
    expect(d2Spy.compileCalls).toBe(0);
  });

  it("carries the board graph and the address of the rest", async () => {
    const { manifest } = serveBoards("graph -> board");
    const html = await withPrerender(() =>
      renderLesson(walkthroughOf("graph -> board"), LESSON_CTX),
    );
    expect(JSON.parse(decodeAttr(html, "data-boards")!)).toEqual(manifest);
    expect(decodeAttr(html, "data-fence")).toBe("url-shortener");
    expect(decodeAttr(html, "data-lesson")).toBe(LESSON_CTX.lessonPath);
  });

  it("fetches the ROOT board only — the rest are a click the reader may never take", async () => {
    const { seen } = serveBoards("one -> fetch");
    await withPrerender(() => renderLesson(walkthroughOf("one -> fetch"), LESSON_CTX));
    expect(seen.filter((url) => url.includes(".svg"))).toHaveLength(1);
    expect(seen.some((url) => url.includes("root.svg"))).toBe(true);
    expect(seen.some((url) => url.includes("a.svg"))).toBe(false);
  });

  it("falls back to the source when the lesson is unknown, without asking the server", async () => {
    // The authoring preview and the blog render through this pipeline with no lesson at all.
    const { seen } = serveBoards("no -> lesson");
    const html = await withPrerender(() => renderLesson(walkthroughOf("no -> lesson")));
    expect(html).toContain('class="d2-boards"');
    expect(html).not.toContain("data-prerendered");
    expect(seen).toHaveLength(0);
  });

  it("falls back when the repo has not drawn it yet", async () => {
    serveBoards("not -> drawn", false);
    const html = await withPrerender(() => renderLesson(walkthroughOf("not -> drawn"), LESSON_CTX));
    expect(html).toContain('class="d2-boards"');
    expect(html).not.toContain("data-prerendered");
    expect(decodeAttr(html, "data-source")).toBe("not -> drawn");
  });

  it("refuses a manifest it cannot read rather than painting half a viewer", async () => {
    const bodies = ["<!doctype html>", "{}", '{"generator":99,"boards":[]}', "{"];
    for (const [i, body] of bodies.entries()) {
      const source = `bad -> manifest ${i}`;
      vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, text: async () => body })));
      const html = await withPrerender(() => renderLesson(walkthroughOf(source), LESSON_CTX));
      expect(html, body).not.toContain("data-prerendered");
      expect(html, body).toContain('class="d2-boards"');
    }
  });

  it("ignores boards drawn from a source the fence no longer holds", async () => {
    // CI has not caught up with the edit. Serving the drawn boards would show the reader the
    // PREVIOUS diagram with no error at all; the client draws what the author actually wrote.
    serveBoards("the -> old source");
    const html = await withPrerender(() =>
      renderLesson(walkthroughOf("the -> new source"), LESSON_CTX),
    );
    expect(html).not.toContain("data-prerendered");
    expect(decodeAttr(html, "data-source")).toBe("the -> new source");
  });

  it("never joins a slideshow run, on either side", async () => {
    const walkthrough = walkthroughOf("alone -> here");
    const html = await renderLesson(
      `\`\`\`d2\na -> b\n\`\`\`\n${walkthrough}\n\`\`\`d2\nc -> d\n\`\`\``,
    );
    expect(html).toContain('class="d2-boards"');
    // The neighbours stay lone blocks: grouping them would make the walkthrough a dead slide.
    expect(html).not.toContain("d2-slideshow");
    expect(html.match(/class="d2-block"/g) ?? []).toHaveLength(2);
  });

  it("stamps every figure with the position of the fence it came from", async () => {
    // The Edit affordance points at a fence by ORDINAL, so these numbers are the whole contract
    // between a rendered figure and the markdown it can be edited back into. A run of adjacent
    // fences is one card carrying several, which is why it also says how many.
    const html = await renderLesson(
      [
        "```d2\nfirst -> one\n```", // fence 0
        "text",
        "```d2\nrun -> a\n```\n```d2\nrun -> b\n```", // fences 1 and 2, one card
        "text",
        walkthroughOf("walk -> through"), // fence 3
        "text",
        "```d2\nlast -> one\n```", // fence 4
      ].join("\n\n"),
    );
    const at = [...html.matchAll(/data-fence-at="(\d+)"/g)].map((m) => Number(m[1]));
    expect(at).toEqual([0, 1, 3, 4]);
    expect(html).toContain('data-fence-count="2"');
    // …and the walkthrough's index is the one the run does NOT claim.
    expect(/<div class="d2-boards" data-fence-at="3"/.test(html)).toBe(true);
  });

  it("leaves a plain ```d2 fence in the pool", async () => {
    serveMedia();
    const html = await withPrerender(() => renderLesson("```d2\nx -> y\n```", LESSON_CTX));
    expect(html).toContain('class="d2-block"');
    expect(html).not.toContain("d2-boards");
  });
});
