// Spec for ```d2 fences, split out of render.test.ts because they are the one fence family whose
// output depends on WHERE the pipeline runs.
//
// The same `renderLesson` serves two callers: Astro's SSR, which draws the diagrams and ships
// SVG, and the browser (the authoring preview), which cannot and ships source for the client to
// compile. Both shapes are pinned here, along with the fallback that connects them — because the
// fallback is silent by design, a broken SSR path renders a perfectly good page, and only an
// assertion can tell the difference.
import { describe, expect, it, vi } from "vitest";

import { renderLesson } from "./render";
import { decodeAttr } from "./testkit";

// The engine, stubbed. Two things ride on this: `compileCalls` proves the pipeline touches no
// WASM on the paths that must not (the browser, and the kill switch), and a source containing
// `!!` stands in for a malformed diagram so the fallback path is reachable without shipping a
// real 21 MB worker into the unit suite.
const d2Spy = vi.hoisted(() => ({ compileCalls: 0 }));
vi.mock("@terrastruct/d2", () => ({
  D2: class {
    // The real client routes every public method through `this.sendMessage`, so a method pulled
    // off the instance loses its worker and dies. Mirroring that here is deliberate: a detached
    // `const compile = d2.compile` fails in this suite rather than surviving to the e2e.
    async sendMessage(kind: "compile" | "render", payload: unknown) {
      if (kind === "compile") {
        const src = payload as string;
        d2Spy.compileCalls += 1;
        if (src.includes("!!")) throw new Error("d2: parse error");
        return { diagram: { src } };
      }
      const { diagram, options } = payload as {
        diagram: { src: string };
        options: { salt: string };
      };
      return `<svg class="d2" data-salt="${options.salt}">${diagram.src}</svg>`;
    }
    async compile(src: string) {
      return this.sendMessage("compile", src) as Promise<{ diagram: { src: string } }>;
    }
    async render(diagram: { src: string }, options: { salt: string }) {
      return this.sendMessage("render", { diagram, options }) as Promise<string>;
    }
  },
}));

describe("d2 fences → source-carrying placeholders", () => {
  it("a lone ```d2 fence becomes a d2-block carrying the RAW SOURCE", async () => {
    const html = await renderLesson("```d2\nx -> y\n```");
    expect(html).toContain('class="d2-block"');
    expect(decodeAttr(html, "data-source")).toBe("x -> y"); // the raw source, not an SVG
    expect(html).not.toContain("<pre"); // the fence is replaced, not also highlighted
    expect(html).not.toContain("d2-slides");
    expect(d2Spy.compileCalls).toBe(0); // no engine on this path
  });

  it("consecutive ```d2 fences group into ONE d2-slideshow carrying each source", async () => {
    const html = await renderLesson("```d2\na -> b\n```\n\n```d2\nc -> d\n```");
    expect(html).toContain('class="d2-slideshow"');
    expect(html).not.toContain("d2-block");
    const slides = JSON.parse(decodeAttr(html, "data-slides")!) as string[];
    expect(slides).toEqual(["a -> b", "c -> d"]);
  });

  it("a paragraph between two d2 fences breaks the group into two d2-blocks", async () => {
    const html = await renderLesson("```d2\na -> b\n```\n\nBetween.\n\n```d2\nc -> d\n```");
    expect(html).not.toContain("d2-slides");
    expect(html.match(/class="d2-block"/g) ?? []).toHaveLength(2);
  });

  it("never invokes d2 with pre-rendering off — not even on a d2-heavy document", async () => {
    const before = d2Spy.compileCalls;
    await renderLesson("```d2\nx -> y\n```\n\n```mermaid\nflowchart LR\n A-->B\n```");
    expect(d2Spy.compileCalls).toBe(before); // the browser path, and the kill switch, stay clean
  });

  it("a ```D2 fence is a d2 fence — the language is matched case-insensitively", async () => {
    const html = await renderLesson("```D2\nx -> y\n```");
    expect(html).toContain('class="d2-block"');
    expect(html).not.toContain("<pre"); // not silently left as highlighted code
  });
});

describe("d2 fences → server-rendered figures", () => {
  /** Run `body` with pre-rendering forced on, then restore the ambient setting. */
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

  it("inlines the SVG and drops data-source, so nothing can recompile it", async () => {
    const html = await withPrerender(() => renderLesson("```d2\nx -> y\n```"));
    expect(html).toContain('data-prerendered="1"');
    expect(html).toContain('class="diagram__figure"');
    expect(html).toContain("<svg");
    expect(html).not.toContain("data-source"); // the source would be dead weight beside the SVG
  });

  it("salts by CONTENT, so the same diagram keys the same cache entry wherever it appears", async () => {
    const first = await withPrerender(() => renderLesson("```d2\nx -> y\n```"));
    const again = await withPrerender(() => renderLesson("Prose.\n\n```d2\nx -> y\n```"));
    const saltOf = (html: string) => html.match(/data-salt="([^"]*)"/)?.[1];
    expect(saltOf(first)).toBeDefined();
    expect(saltOf(again)).toBe(saltOf(first)); // position-independent
  });

  it("gives a repeated diagram a distinct salt within one document", async () => {
    const html = await withPrerender(() =>
      renderLesson("```d2\nx -> y\n```\n\nBetween.\n\n```d2\nx -> y\n```"),
    );
    const salts = [...html.matchAll(/data-salt="([^"]*)"/g)].map((m) => m[1]);
    expect(salts).toHaveLength(2);
    expect(salts[0]).not.toBe(salts[1]); // duplicate ids in one document would be invalid HTML
  });

  it("falls back to the source placeholder when a diagram will not compile", async () => {
    const html = await withPrerender(() => renderLesson("```d2\n!! nope\n```"));
    expect(html).toContain('class="d2-block"');
    expect(html).not.toContain("data-prerendered");
    expect(decodeAttr(html, "data-source")).toBe("!! nope"); // the client shows the error card
  });

  it("draws a slideshow's FIRST slide only, and still ships every slide's source", async () => {
    const html = await withPrerender(() => renderLesson("```d2\na -> b\n```\n```d2\nc -> d\n```"));
    expect(html).toContain('class="d2-slideshow"');
    expect(html).toContain('data-prerendered="1"');
    // Slide 0 is the one the transport paints at mount, so it is drawn; the rest stay source, to
    // be compiled if and when a reader steps to them.
    expect(html.match(/<svg/g) ?? []).toHaveLength(1);
    expect(html).toContain(">a -> b</svg>"); // the drawn slide is the first one, not the second
    expect(JSON.parse(decodeAttr(html, "data-slides")!)).toEqual(["a -> b", "c -> d"]);
  });

  it("the kill switch restores the placeholder output exactly", async () => {
    const LESSON = "```d2\nx -> y\n```";
    const previous = process.env.SYNAPSE_D2_PRERENDER;
    try {
      process.env.SYNAPSE_D2_PRERENDER = "on";
      const on = await renderLesson(LESSON);
      process.env.SYNAPSE_D2_PRERENDER = "off";
      const off = await renderLesson(LESSON);
      // Both directions, or this passes without the switch doing anything: `off` must be the
      // untouched placeholder, and `on` must actually have differed from it.
      expect(on).not.toBe(off);
      expect(on).toContain("data-prerendered");
      expect(off).toBe(`<div class="d2-block" data-source="${encodeURIComponent("x -> y")}"></div>`);
    } finally {
      if (previous === undefined) delete process.env.SYNAPSE_D2_PRERENDER;
      else process.env.SYNAPSE_D2_PRERENDER = previous;
    }
  });
});
