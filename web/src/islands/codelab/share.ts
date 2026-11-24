/**
 * A share link that carries the whole lab in its FRAGMENT: `#s=<base64url of deflated JSON>`.
 *
 * Nothing is stored anywhere. The fragment never reaches the server (browsers do not send it), so
 * a shared program is not logged, cached or rate-limited by this site — and also cannot be revoked
 * or outlive the URL that holds it. Deflate is the browser's own `CompressionStream`, which keeps
 * this out of the bundle; code compresses well, so a page of source fits a comfortable URL.
 *
 * Decoding never throws: a truncated paste or a hand-edited link answers null and the page says
 * so, because a share link is input from anywhere.
 */

export interface ShareTab {
  readonly name: string;
  readonly language: string;
  readonly source: string;
}

export interface ShareState {
  readonly tabs: readonly ShareTab[];
  /** Index into `tabs` of the one on screen. */
  readonly active: number;
  readonly stdin: string;
}

/** The fragment's key, so the hash can grow other keys without a version bump. */
export const SHARE_PREFIX = "#s=";
const VERSION = 1;

/** Past this the link may be cut short by a chat client or a browser bar; the page warns. */
export const LONG_URL = 8_000;

interface Wire {
  v: number;
  t: [string, string, string][];
  a: number;
  i: string;
}

async function pipe(bytes: Uint8Array<ArrayBuffer>, stream: CompressionStream | DecompressionStream): Promise<Uint8Array<ArrayBuffer>> {
  const out = new Blob([bytes]).stream().pipeThrough(stream);
  return new Uint8Array(await new Response(out).arrayBuffer());
}

function toBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function fromBase64Url(text: string): Uint8Array<ArrayBuffer> {
  const b64 = text.replace(/-/g, "+").replace(/_/g, "/");
  const binary = atob(b64 + "=".repeat((4 - (b64.length % 4)) % 4));
  return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}

/** The fragment for `state`, `#s=` included. */
export async function encodeShare(state: ShareState): Promise<string> {
  const wire: Wire = {
    v: VERSION,
    t: state.tabs.map((tab) => [tab.name, tab.language, tab.source]),
    a: state.active,
    i: state.stdin,
  };
  const bytes = new TextEncoder().encode(JSON.stringify(wire));
  return SHARE_PREFIX + toBase64Url(await pipe(bytes, new CompressionStream("deflate-raw")));
}

/** The state a fragment carries, or null when it is not a share link or will not decode. */
export async function decodeShare(hash: string): Promise<ShareState | null> {
  if (!hash.startsWith(SHARE_PREFIX)) return null;
  try {
    const bytes = await pipe(fromBase64Url(hash.slice(SHARE_PREFIX.length)), new DecompressionStream("deflate-raw"));
    const wire = JSON.parse(new TextDecoder().decode(bytes)) as Partial<Wire>;
    if (wire.v !== VERSION || !Array.isArray(wire.t)) return null;
    const tabs = wire.t
      .filter((row): row is [string, string, string] => Array.isArray(row) && row.every((cell) => typeof cell === "string"))
      .map(([name, language, source]) => ({ name, language, source }));
    if (tabs.length === 0) return null;
    const active = typeof wire.a === "number" && wire.a >= 0 && wire.a < tabs.length ? wire.a : 0;
    return { tabs, active, stdin: typeof wire.i === "string" ? wire.i : "" };
  } catch {
    return null;
  }
}
