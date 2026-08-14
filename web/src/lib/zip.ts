// ──────────────────────────────────────────────────────────────────
// ZIP, STORED — enough of the format to hand someone a folder
// ──────────────────────────────────────────────────────────────────
// The `/d2` editor exports a walkthrough's `_d2/<name>/` directory: one manifest and one SVG per
// board, as a single download an author drops beside a lesson.
//
// STORED, not deflated, which is a legal archive every unzip accepts — and the reason there is no
// dependency here. `CompressionStream` could deflate, but the payload is a handful of SVGs that
// the browser is about to write to disk once, so a compression library (or a stream dance) buys
// nothing a user would notice.
//
// Scope is exactly what that needs: no directories, no zip64, no encryption. Files must stay
// under 4 GiB and count under 65535 — both true by construction for a diagram.

/** One entry: a name relative to the archive root, and its bytes. */
export interface ZipEntry {
  name: string;
  content: string;
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let i = 0; i < 256; i += 1) {
    let c = i;
    for (let bit = 0; bit < 8; bit += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[i] = c >>> 0;
  }
  return table;
})();

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) crc = CRC_TABLE[(crc ^ byte) & 0xff]! ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

/** A little-endian writer over a growing byte list — the whole format is LE fixed-width fields. */
class Bytes {
  // Pinned to a plain ArrayBuffer rather than the default `ArrayBufferLike`: a `Blob` part cannot
  // be backed by a SharedArrayBuffer, and the wider type is what makes that a compile error here
  // instead of a runtime one.
  private readonly parts: Uint8Array<ArrayBuffer>[] = [];
  length = 0;

  push(chunk: Uint8Array<ArrayBuffer>): void {
    this.parts.push(chunk);
    this.length += chunk.length;
  }

  u16(value: number): void {
    this.push(new Uint8Array([value & 0xff, (value >>> 8) & 0xff]));
  }

  u32(value: number): void {
    this.push(
      new Uint8Array([value & 0xff, (value >>> 8) & 0xff, (value >>> 16) & 0xff, (value >>> 24) & 0xff]),
    );
  }

  concat(): Uint8Array<ArrayBuffer> {
    const out = new Uint8Array(this.length);
    let at = 0;
    for (const part of this.parts) {
      out.set(part, at);
      at += part.length;
    }
    return out;
  }
}

/**
 * A stored (uncompressed) ZIP of `entries`.
 *
 * Names are written as UTF-8 with the language-encoding flag set, so a board titled in any script
 * survives the round trip — a name is a slug today, but the flag costs one bit and removes a
 * whole class of mojibake later.
 */
export function zip(entries: ZipEntry[]): Blob {
  const encoder = new TextEncoder();
  const local = new Bytes();
  const central = new Bytes();
  // No timestamp: a deterministic archive means re-exporting an unchanged diagram gives an
  // identical file, and `Date` would be the only thing making it differ.
  const DOS_TIME = 0;
  const DOS_DATE = 0x21; // 1980-01-01, the epoch the format starts at
  const UTF8_FLAG = 0x800;

  for (const entry of entries) {
    const name = encoder.encode(entry.name);
    const body = encoder.encode(entry.content);
    const sum = crc32(body);
    const offset = local.length;

    local.u32(0x04034b50); // local file header
    local.u16(20); // version needed
    local.u16(UTF8_FLAG);
    local.u16(0); // stored
    local.u16(DOS_TIME);
    local.u16(DOS_DATE);
    local.u32(sum);
    local.u32(body.length);
    local.u32(body.length);
    local.u16(name.length);
    local.u16(0); // extra field
    local.push(name);
    local.push(body);

    central.u32(0x02014b50); // central directory header
    central.u16(20); // version made by
    central.u16(20); // version needed
    central.u16(UTF8_FLAG);
    central.u16(0); // stored
    central.u16(DOS_TIME);
    central.u16(DOS_DATE);
    central.u32(sum);
    central.u32(body.length);
    central.u32(body.length);
    central.u16(name.length);
    central.u16(0); // extra
    central.u16(0); // comment
    central.u16(0); // disk
    central.u16(0); // internal attrs
    central.u32(0); // external attrs
    central.u32(offset);
    central.push(name);
  }

  const end = new Bytes();
  end.u32(0x06054b50); // end of central directory
  end.u16(0); // this disk
  end.u16(0); // disk with the central directory
  end.u16(entries.length);
  end.u16(entries.length);
  end.u32(central.length);
  end.u32(local.length);
  end.u16(0); // comment length

  return new Blob([local.concat(), central.concat(), end.concat()], { type: "application/zip" });
}

/** Hand a blob to the browser as a download. */
export function download(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  // Revoked on the next turn: revoking synchronously can beat the navigation the click starts.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
