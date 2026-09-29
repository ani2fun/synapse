import { describe, expect, it } from "vitest";

import { renderNotes } from "./render";

describe("renderNotes", () => {
  it("renders the GFM core", async () => {
    const html = await renderNotes("## Plan\n\n- [ ] two pointers\n\n**fast** ~~slow~~");
    expect(html).toContain("<h2>Plan</h2>");
    expect(html).toContain('type="checkbox"');
    expect(html).toContain("<strong>fast</strong>");
    expect(html).toContain("<del>slow</del>");
  });

  it("drops raw HTML instead of passing it through", async () => {
    const html = await renderNotes('hi <img src=x onerror="alert(1)"> there\n\n<script>alert(1)</script>');
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<script");
    expect(html).not.toContain("onerror");
  });

  it("removes unsafe link and image targets, including obfuscated schemes", async () => {
    const html = await renderNotes("[a](javascript:alert(1)) [b](java\tscript:x) ![c](data:text/html,x)");
    expect(html).not.toMatch(/javascript|data:/i);
  });

  it("keeps safe links and opens them away from the problem page", async () => {
    const html = await renderNotes("[docs](https://example.com)");
    expect(html).toContain('href="https://example.com"');
    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
  });

  it("leaves fences as plain code, never a live widget", async () => {
    const html = await renderNotes("```python run\nprint(1)\n```");
    expect(html).toContain("<pre><code");
    expect(html).not.toContain("workbench");
  });
});
