// Lesson-local images (ADR-RS013): a relative `_assets/…` image src names a file beside the lesson,
// so the renderer rewrites it to the route that serves that file for the page being rendered.
import { describe, expect, it } from "vitest";

import { localImageUrl, renderLesson } from "./render";

const PAGE = "learn/building-synapse/the-system-in-c4/context";

describe("lesson-local images", () => {
  it("rewrites an _assets image to its /content-assets URL", async () => {
    const html = await renderLesson("![Keycloak login](_assets/keycloak-login.png)", { slugPath: PAGE });
    expect(html).toContain(`src="/content-assets/${PAGE}/_assets/keycloak-login.png"`);
    expect(html).toContain('alt="Keycloak login"');
  });

  it("rewrites images nested in lists and links, and a ./ prefix", async () => {
    const html = await renderLesson("- [![a](./_assets/screens/a.svg)](https://example.com)", { slugPath: PAGE });
    expect(html).toContain(`src="/content-assets/${PAGE}/_assets/screens/a.svg"`);
  });

  it("rewrites every frame of a stepping figure", async () => {
    const md = ["![Flow — frame 1 of 2](_assets/f1.png)", "", "![Flow — frame 2 of 2](_assets/f2.png)"].join("\n");
    const html = await renderLesson(md, { slugPath: PAGE });
    const frames = JSON.parse(decodeURIComponent(/data-frames="([^"]*)"/.exec(html)![1]));
    expect(frames).toEqual([`/content-assets/${PAGE}/_assets/f1.png`, `/content-assets/${PAGE}/_assets/f2.png`]);
  });

  it("leaves other srcs, traversal, and pages without a slug path alone", () => {
    expect(localImageUrl("/media/java/x.png", PAGE)).toBe("/media/java/x.png");
    expect(localImageUrl("https://example.com/_assets/x.png", PAGE)).toBe("https://example.com/_assets/x.png");
    expect(localImageUrl("_assets/../01-intro.md", PAGE)).toBe("_assets/../01-intro.md");
    expect(localImageUrl("_assets/x.png", undefined)).toBe("_assets/x.png");
  });
});
