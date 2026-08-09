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
      expect(off).toBe(`<div class="d2-block" data-source="${encodeURIComponent("x -> y")}"></div>`);
    } finally {
      if (previous === undefined) delete process.env.SYNAPSE_D2_PRERENDER;
      else process.env.SYNAPSE_D2_PRERENDER = previous;
    }
  });
});
