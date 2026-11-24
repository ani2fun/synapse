import { describe, expect, it } from "vitest";

import { decodeShare, encodeShare, SHARE_PREFIX } from "./share";
import { parseSettings, DEFAULT_SETTINGS, MAX_FONT_SIZE } from "./settings";

const STATE = {
  tabs: [
    { name: "Code 1", language: "python", source: 'print(input("Name? "))\n' },
    { name: "Ünïcode ✓", language: "go", source: "package main\n\nfunc main() {}\n" },
  ],
  active: 1,
  stdin: "Ada\n",
};

describe("share links", () => {
  it("round-trips the tabs, the tab on screen and the input", async () => {
    const hash = await encodeShare(STATE);
    expect(hash.startsWith(SHARE_PREFIX)).toBe(true);
    // base64url: safe in a URL with no escaping.
    expect(hash.slice(SHARE_PREFIX.length)).toMatch(/^[A-Za-z0-9_-]+$/);
    expect(await decodeShare(hash)).toEqual(STATE);
  });

  it("answers null, never throws, for anything that is not a good link", async () => {
    const hash = await encodeShare(STATE);
    expect(await decodeShare("")).toBeNull();
    expect(await decodeShare("#other=1")).toBeNull();
    expect(await decodeShare(hash.slice(0, hash.length - 12))).toBeNull();
    expect(await decodeShare(`${SHARE_PREFIX}!!!not-base64`)).toBeNull();
  });
});

describe("lab settings", () => {
  it("keeps every good field and replaces only the bad ones", () => {
    expect(parseSettings(JSON.stringify({ theme: "vs-dark", tabSize: 3, fontSize: 99 }))).toEqual({
      ...DEFAULT_SETTINGS,
      theme: "vs-dark",
      fontSize: MAX_FONT_SIZE,
    });
    expect(parseSettings("{nope")).toEqual(DEFAULT_SETTINGS);
    expect(parseSettings(null)).toEqual(DEFAULT_SETTINGS);
  });
});
