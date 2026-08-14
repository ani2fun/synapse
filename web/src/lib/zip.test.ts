// The archive the `/d2` editor hands an author. It is written by hand, so the parts a real unzip
// checks — the signatures, the CRCs, the offsets that let it find each entry — are asserted here
// rather than trusted. A malformed archive fails at the user's shell, not in this process.
import { describe, expect, it } from "vitest";

import { type ZipEntry, zip } from "./zip";

const LOCAL = 0x04034b50;
const CENTRAL = 0x02014b50;
const END = 0x06054b50;

async function bytesOf(entries: ZipEntry[]): Promise<DataView> {
  return new DataView(await zip(entries).arrayBuffer());
}

/** The end-of-central-directory record, read from the tail like an unzip does. */
function endRecord(view: DataView) {
  const at = view.byteLength - 22; // no archive comment, so it is exactly the last 22 bytes
  expect(view.getUint32(at, true)).toBe(END);
  return {
    count: view.getUint16(at + 10, true),
    centralSize: view.getUint32(at + 12, true),
    centralOffset: view.getUint32(at + 16, true),
  };
}

/** Walk the central directory and return each entry's name, size, CRC and local offset. */
function centralEntries(view: DataView) {
  const { count, centralOffset } = endRecord(view);
  const decoder = new TextDecoder();
  const out: { name: string; crc: number; size: number; offset: number }[] = [];
  let at = centralOffset;
  for (let i = 0; i < count; i += 1) {
    expect(view.getUint32(at, true)).toBe(CENTRAL);
    const nameLen = view.getUint16(at + 28, true);
    out.push({
      crc: view.getUint32(at + 16, true),
      size: view.getUint32(at + 24, true),
      offset: view.getUint32(at + 42, true),
      name: decoder.decode(new Uint8Array(view.buffer, at + 46, nameLen)),
    });
    at += 46 + nameLen + view.getUint16(at + 30, true) + view.getUint16(at + 32, true);
  }
  return out;
}

/** The bytes an unzip would extract for one central-directory entry. */
function extract(view: DataView, entry: { offset: number; size: number }): string {
  expect(view.getUint32(entry.offset, true)).toBe(LOCAL);
  const nameLen = view.getUint16(entry.offset + 26, true);
  const extraLen = view.getUint16(entry.offset + 28, true);
  const start = entry.offset + 30 + nameLen + extraLen;
  return new TextDecoder().decode(new Uint8Array(view.buffer, start, entry.size));
}

const SIDECAR: ZipEntry[] = [
  { name: "boards.json", content: '{"generator":1,"root":"root"}' },
  { name: "root.svg", content: "<svg>root</svg>" },
  { name: "container.svg", content: "<svg>container</svg>" },
];

describe("the stored zip", () => {
  it("round-trips a walkthrough's sidecar", async () => {
    const view = await bytesOf(SIDECAR);
    const entries = centralEntries(view);
    expect(entries.map((e) => e.name)).toEqual(SIDECAR.map((e) => e.name));
    for (const [i, entry] of entries.entries()) {
      expect(extract(view, entry), entry.name).toBe(SIDECAR[i]!.content);
    }
  });

  it("agrees with itself about where everything is", async () => {
    // The three numbers an unzip trusts before reading a single byte of content.
    const view = await bytesOf(SIDECAR);
    const { count, centralSize, centralOffset } = endRecord(view);
    expect(count).toBe(SIDECAR.length);
    expect(centralOffset + centralSize).toBe(view.byteLength - 22);
  });

  it("computes a CRC that matches the bytes it stored", async () => {
    // A known value, so a rewrite of the table cannot pass by being self-consistent.
    const view = await bytesOf([{ name: "a.txt", content: "123456789" }]);
    expect(centralEntries(view)[0]!.crc).toBe(0xcbf43926);
  });

  it("handles an empty file", async () => {
    const view = await bytesOf([{ name: "empty.svg", content: "" }]);
    const [entry] = centralEntries(view);
    expect(entry!.size).toBe(0);
    expect(entry!.crc).toBe(0);
    expect(extract(view, entry!)).toBe("");
  });

  it("stores a unicode name as UTF-8 and says so", async () => {
    const view = await bytesOf([{ name: "café-☕.svg", content: "<svg/>" }]);
    expect(centralEntries(view)[0]!.name).toBe("café-☕.svg");
    // Bit 11 tells the reader the name is UTF-8 rather than the format's default code page.
    const { centralOffset } = endRecord(view);
    expect(view.getUint16(centralOffset + 8, true) & 0x800).toBe(0x800);
  });

  it("handles an entry past a single buffer's worth", async () => {
    const big = "<svg>".repeat(20_000); // ~100 KB, well past any chunking
    const view = await bytesOf([{ name: "big.svg", content: big }]);
    const [entry] = centralEntries(view);
    expect(entry!.size).toBe(new TextEncoder().encode(big).length);
    expect(extract(view, entry!)).toBe(big);
  });

  it("is deterministic, so re-exporting an unchanged diagram gives the same file", async () => {
    const [a, b] = [await zip(SIDECAR).arrayBuffer(), await zip(SIDECAR).arrayBuffer()];
    expect(new Uint8Array(a)).toEqual(new Uint8Array(b));
  });
});
