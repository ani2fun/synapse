// Lesson-local simulators (ADR-RS012): the URL a widget beside its lesson is served from, and the
// self-contained document a private book's widget is framed as.
import { describe, expect, it } from "vitest";

import { contentAssetUrl, selfContained } from "./simulatorDoc";

const ORIGIN = "https://synapse.test";

describe("contentAssetUrl", () => {
  it("resolves against the reader URL's slug path", () => {
    expect(contentAssetUrl("_assets/_simulators/index.html?fig=04", "/synapse/dsa/recursion/head/head")).toBe(
      "/content-assets/dsa/recursion/head/head/_assets/_simulators/index.html?fig=04",
    );
  });

  it("ignores a trailing slash on the page URL", () => {
    expect(contentAssetUrl("_assets/_simulators/a.html", "/synapse/dsa/lesson/")).toBe(
      "/content-assets/dsa/lesson/_assets/_simulators/a.html",
    );
  });

  it("has nothing to resolve against off a reader page", () => {
    expect(contentAssetUrl("_assets/_simulators/a.html", "/edit/dsa/lesson")).toBeNull();
    expect(contentAssetUrl("_assets/_simulators/a.html", "/synapse/")).toBeNull();
  });
});

describe("selfContained", () => {
  const files: Record<string, string> = {
    "/content-assets/dsa/lesson/_assets/_simulators/index.html":
      '<!doctype html>\n<link rel="stylesheet" href="../../../_assets/_simulators/runtime/figures.css">\n' +
      '<script src="/content-assets/dsa/_assets/_simulators/runtime/figures.js"></script>\n' +
      '<script src="./figures.js"></script>\n<script src="https://cdn.example/lib.js"></script>\n' +
      "<body><script>show()</script></body>",
    "/content-assets/dsa/_assets/_simulators/runtime/figures.js": "var runtime = '$&'; // </script> in a string",
    "/content-assets/dsa/lesson/_assets/_simulators/figures.js": "var figures = 1;",
    "/content-assets/dsa/_assets/_simulators/runtime/figures.css": "svg { width: 100% }",
  };
  const fetched: string[] = [];
  const fetchText = async (path: string): Promise<string> => {
    fetched.push(path);
    const text = files[path];
    if (text == null) throw new Error(`no ${path}`);
    return text;
  };

  it("inlines same-origin scripts and styles in order, defining the params first", async () => {
    const doc = await selfContained("/content-assets/dsa/lesson/_assets/_simulators/index.html?fig=04", ORIGIN, fetchText);
    expect(doc.startsWith("<!doctype html>")).toBe(true); // the params script must not precede the doctype
    const params = doc.indexOf("window.synapseSimulator");
    const runtime = doc.indexOf("var runtime");
    const figures = doc.indexOf("var figures");
    expect(params).toBeGreaterThan(0);
    expect(params).toBeLessThan(runtime);
    expect(runtime).toBeLessThan(figures);
    expect(doc).toContain('new URLSearchParams("?fig=04")');
    expect(doc).toContain("<style>svg { width: 100% }</style>");
    expect(doc).not.toContain('src="./figures.js"');
  });

  it("keeps code literal: `$&` survives and `</script>` cannot close its element early", async () => {
    const doc = await selfContained("/content-assets/dsa/lesson/_assets/_simulators/index.html", ORIGIN, fetchText);
    expect(doc).toContain("var runtime = '$&';");
    expect(doc).toContain("<\\/script> in a string");
  });

  it("leaves a cross-origin script alone and never fetches it", async () => {
    const doc = await selfContained("/content-assets/dsa/lesson/_assets/_simulators/index.html", ORIGIN, fetchText);
    expect(doc).toContain('src="https://cdn.example/lib.js"');
    expect(fetched.some((path) => path.includes("cdn.example"))).toBe(false);
  });

  it("fails when a file is refused, so the card can say so", async () => {
    await expect(selfContained("/content-assets/dsa/other/_assets/_simulators/index.html", ORIGIN, fetchText)).rejects.toThrow();
  });
});
