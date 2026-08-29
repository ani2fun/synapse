// Spec for ```d2 fences, split out of render.test.ts because they are the one fence family whose
// output depends on WHERE the pipeline runs and on whether a renderer is reachable.
//
// The same `renderLesson` serves two callers. Under SSR it hands each diagram to the `d2-render`
// sidecar and inlines what comes back. In the browser (the authoring preview) it cannot, and ships
// the source for the client to compile. Both shapes are pinned here, along with the fallback that
// connects them: a fence the sidecar could not draw renders a perfectly good page the slow way, so
// only an assertion can tell a drawn figure from a silently undrawn one.
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

/**
 * Stand in for the render sidecar. `drawn` decides which diagrams it agrees to draw — a `false` is
 * every way the real one can decline at once: unreachable, timed out, or a diagram it cannot parse.
 *
 * The request carries the SOURCE, not a hash: the sidecar addresses its own cache and the caller
 * has no filename to ask for any more. `seen` collects the hashes so a test can still assert how
 * many distinct diagrams were requested.
 */
function serveRenderer(drawn: (hash: string) => boolean = () => true) {
  const seen: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: { body?: string }) => {
      if (!String(url).endsWith("/render")) return { ok: false, text: async () => "" };
      const source = String(JSON.parse(init?.body ?? "{}").source ?? "");
      const hash = fnv1a(source);
      seen.push(hash);
      if (!drawn(hash)) return { ok: false, text: async () => "" };
      // Shaped like the real thing: the salt baked in is the diagram's FIRST-occurrence salt.
      return { ok: true, text: async () => `<svg data-salt="d2-${hash}">figure ${hash}</svg>` };
    }),
  );
  return seen;
}

/** A renderer address that no test ever connects to — `fetch` is stubbed. Its only job is to make
 *  `rendererUrl()` non-null, which is what turns server-side drawing on. */
const RENDERER = "http://d2-render.test";

/** Run `body` with a renderer configured, then restore the ambient setting. */
async function withPrerender<T>(body: () => Promise<T>): Promise<T> {
  const previous = process.env.SYNAPSE_D2_RENDER_URL;
  process.env.SYNAPSE_D2_RENDER_URL = RENDERER;
  try {
    return await body();
  } finally {
    if (previous === undefined) delete process.env.SYNAPSE_D2_RENDER_URL;
    else process.env.SYNAPSE_D2_RENDER_URL = previous;
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
    serveRenderer();
    const html = await withPrerender(() => renderLesson("```d2\nx -> y\n```"));
    expect(html).toContain('data-prerendered="1"');
    expect(html).toContain('class="diagram__figure"');
    expect(html).toContain("<svg");
    expect(html).not.toContain("data-source"); // the source would be dead weight beside the SVG
    expect(d2Spy.compileCalls).toBe(0); // inlined a file; compiled nothing
  });

  // Each of these uses its OWN diagram: the lookup cache is module-level and outlives a test, so
  // reusing a source would serve a neighbour's answer and assert nothing.
  it("keys a diagram by CONTENT, so one render serves it wherever it appears", async () => {
    const source = "content -> addressed";
    const asked = serveRenderer();
    await withPrerender(() => renderLesson(`\`\`\`d2\n${source}\n\`\`\``));
    await withPrerender(() => renderLesson(`Prose first.\n\n\`\`\`d2\n${source}\n\`\`\``));
    // Same source → same key, independent of position, and the second document is served from
    // this process's cache without asking again. That is what keeps a repeated diagram — across
    // documents, not just within one — a single render for the whole catalog's lifetime.
    expect(asked).toEqual([fnv1a(source)]);
  });

  it("re-salts a repeat within one document so element ids stay unique", async () => {
    serveRenderer();
    const html = await withPrerender(() =>
      renderLesson("```d2\nx -> y\n```\n\nBetween.\n\n```d2\nx -> y\n```"),
    );
    const salts = [...html.matchAll(/data-salt="([^"]*)"/g)].map((m) => m[1]);
    expect(salts).toHaveLength(2);
    expect(salts[0]).toBe(`d2-${fnv1a("x -> y")}`);
    expect(salts[1]).toBe(`d2-${fnv1a("x -> y")}-2`); // the file's salt, rewritten for the copy
  });

  it("falls back to the source placeholder when nobody has drawn the diagram yet", async () => {
    serveRenderer(() => false); // a fence newer than its repo's last CI run
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
    serveRenderer();
    const html = await withPrerender(() => renderLesson("```d2\na -> b\n```\n```d2\nc -> d\n```"));
    expect(html).toContain('class="d2-slideshow"');
    expect(html).toContain('data-prerendered="1"');
    // Slide 0 is the one the transport paints at mount; the rest stay source until stepped to.
    expect(html.match(/<svg/g) ?? []).toHaveLength(1);
    expect(html).toContain(fnv1a("a -> b"));
    expect(JSON.parse(decodeAttr(html, "data-slides")!)).toEqual(["a -> b", "c -> d"]);
  });

  it("the kill switch restores the placeholder output exactly", async () => {
    serveRenderer();
    const LESSON = "```d2\nx -> y\n```";
    const previous = process.env.SYNAPSE_D2_RENDER_URL;
    try {
      process.env.SYNAPSE_D2_RENDER_URL = RENDERER;
      const on = await renderLesson(LESSON);
      // Emptying the address is the kill switch: no renderer, so the reader compiles.
      process.env.SYNAPSE_D2_RENDER_URL = "";
      const off = await renderLesson(LESSON);
      // Both directions, or this passes without the switch doing anything.
      expect(on).not.toBe(off);
      expect(on).toContain("data-prerendered");
      expect(off).toBe(
        `<div class="d2-block" data-fence-at="0" data-source="${encodeURIComponent("x -> y")}"></div>`,
      );
    } finally {
      if (previous === undefined) delete process.env.SYNAPSE_D2_RENDER_URL;
      else process.env.SYNAPSE_D2_RENDER_URL = previous;
    }
  });
});

// ── WALKTHROUGHS ─────────────────────────────────────────────────────────────────────────────
// A ```d2 boards fence is one source that compiles to a TREE of boards, drawn by the renderer and
// addressed by CONTENT like every other figure — which is what retired the lesson-relative `_d2/`
// sidecars, and `RenderContext` with them. Everything here turns on a single rule: the ELEMENT is
// chosen by the fence, and only the FIGURE by whether the render succeeded. Get that backwards and
// every miss — a dead sidecar, the authoring preview, the kill switch — silently degrades to a
// single root board with dead links, which is the bug this replaces.

/** A walkthrough fence. Every case gets its OWN source: the SSR cache is module-level and keyed by
 *  source, exactly as it is in the running server, so sharing one would make these
 *  order-dependent. */
const walkthroughOf = (source: string) =>
  `\`\`\`d2 boards name="url-shortener" root="Context"\n${source}\n\`\`\``;

/** Stand in for the sidecar's `/boards`. `drawn` decides whether it agrees to draw this one. */
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
    vi.fn(async (url: string, init?: { body?: string }) => {
      seen.push(String(url));
      if (!drawn) return { ok: false, json: async () => ({}) };
      const asked = String(JSON.parse(init?.body ?? "{}").source ?? "");
      // The sidecar answers for whatever it was handed; the manifest is built from the source the
      // TEST named, so a mismatch between the two is what the staleness case exercises.
      void asked;
      return { ok: true, json: async () => ({ manifest, rootSvg: "<svg>root board</svg>" }) };
    }),
  );
  return { seen, manifest };
}

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
    const warm = await withPrerender(() => renderLesson(walkthroughOf("warm -> board")));
    expect(warm).toContain('class="d2-boards"');
    expect(warm).toContain('data-prerendered="1"');
    expect(warm).toContain("<svg>root board</svg>");
    expect(warm).not.toContain("data-source"); // nothing may recompile a drawn walkthrough
    expect(d2Spy.compileCalls).toBe(0);
  });

  it("carries the board graph, which is the whole address of the rest", async () => {
    const { manifest } = serveBoards("graph -> board");
    const html = await withPrerender(() => renderLesson(walkthroughOf("graph -> board")));
    expect(JSON.parse(decodeAttr(html, "data-boards")!)).toEqual(manifest);
    // The manifest's `source` IS the address: the viewer fetches /d2-board/<source>/<slug>. No
    // fence name and no lesson path travel with it any more — the same walkthrough in two lessons
    // is one set of boards.
    expect(manifest.source).toBe(fnv1a("graph -> board"));
    expect(html).not.toContain("data-fence=");
    expect(html).not.toContain("data-lesson=");
  });

  it("asks once and inlines the ROOT only — the rest are a click the reader may never take", async () => {
    const { seen } = serveBoards("one -> request");
    const html = await withPrerender(() => renderLesson(walkthroughOf("one -> request")));
    expect(seen).toHaveLength(1);
    expect(seen[0]).toMatch(/\/boards$/);
    // One figure in the HTML, though the manifest names two boards.
    expect(html.match(/<svg/g) ?? []).toHaveLength(1);
  });

  it("falls back with no renderer configured, without asking anything", async () => {
    // The authoring preview and the blog render through this pipeline with no renderer at all.
    const { seen } = serveBoards("no -> renderer");
    const html = await renderLesson(walkthroughOf("no -> renderer"));
    expect(html).toContain('class="d2-boards"');
    expect(html).not.toContain("data-prerendered");
    expect(decodeAttr(html, "data-source")).toBe("no -> renderer");
    expect(seen).toHaveLength(0);
  });

  it("falls back when the renderer declines", async () => {
    serveBoards("not -> drawn", false);
    const html = await withPrerender(() => renderLesson(walkthroughOf("not -> drawn")));
    expect(html).toContain('class="d2-boards"');
    expect(html).not.toContain("data-prerendered");
    expect(decodeAttr(html, "data-source")).toBe("not -> drawn");
  });

  it("refuses a manifest it cannot read rather than painting half a viewer", async () => {
    const payloads: unknown[] = [
      null,
      {},
      { manifest: { generator: 99, boards: [] }, rootSvg: "<svg/>" },
      { manifest: { generator: 1, source: "x", root: "root", boards: [] }, rootSvg: "<svg/>" },
      { manifest: null, rootSvg: "<svg/>" },
    ];
    for (const [i, payload] of payloads.entries()) {
      const source = `bad -> manifest ${i}`;
      vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, json: async () => payload })));
      const html = await withPrerender(() => renderLesson(walkthroughOf(source)));
      expect(html, JSON.stringify(payload)).not.toContain("data-prerendered");
      expect(html, JSON.stringify(payload)).toContain('class="d2-boards"');
    }
  });

  it("refuses a root board that is not an SVG", async () => {
    // A proxy or an error page answering in the sidecar's place must be a miss, never a figure
    // made of someone else's markup.
    const source = "not -> an svg";
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        json: async () => ({
          manifest: {
            generator: 1,
            source: fnv1a(source),
            root: "root",
            boards: [{ id: "root", slug: "root", title: "C", parent: null, links: [] }],
            warnings: [],
          },
          rootSvg: "<!doctype html><html>nope</html>",
        }),
      })),
    );
    const html = await withPrerender(() => renderLesson(walkthroughOf(source)));
    expect(html).not.toContain("data-prerendered");
    expect(html).not.toContain("doctype");
  });

  it("ignores boards drawn from a source the fence no longer holds", async () => {
    // Serving them would show the reader the PREVIOUS diagram with no error at all; the client
    // draws what the author actually wrote.
    serveBoards("the -> old source");
    const html = await withPrerender(() => renderLesson(walkthroughOf("the -> new source")));
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
    serveRenderer();
    const html = await withPrerender(() => renderLesson("```d2\nx -> y\n```"));
    expect(html).toContain('class="d2-block"');
    expect(html).not.toContain("d2-boards");
  });
});
