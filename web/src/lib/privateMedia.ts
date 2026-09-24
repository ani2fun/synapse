// A PRIVATE book's `/media/…` files, made showable.
//
// The server gates a private book's media behind the reader's bearer, the way it gates the prose,
// and an `<img>` cannot send one. So the private lesson island installs a resolver here: it fetches
// a file WITH the bearer and hands back a blob URL the browser can show. Anything that sets an
// image's `src` after the island has run (a frame slideshow stepping to its next frame) asks this
// module instead of using the raw path. With no resolver installed (every public page) each path
// resolves to itself, so public media never pays for the detour.

type Fetcher = (url: string) => Promise<string>;

let fetcher: Fetcher | null = null;
/** One promise per path, so a frame asked for twice (shown, then warmed) is fetched once. */
const resolved = new Map<string, Promise<string>>();

/** Route every later `/media/…` lookup through `fetchBlobUrl`. */
export function installPrivateMedia(fetchBlobUrl: Fetcher): void {
  if (fetcher === fetchBlobUrl) return; // a second body on the same page: keep what it fetched
  fetcher = fetchBlobUrl;
  resolved.clear();
}

/** Whether a resolver is installed, i.e. whether a raw `/media/…` path would be refused. */
export function privateMediaActive(): boolean {
  return fetcher != null;
}

/**
 * A URL the browser can show for `url`: itself when no resolver is installed or it is not a
 * `/media/…` path, otherwise the resolver's blob URL. A refused or missing file resolves to the
 * original path, so the broken-image mark still says what happened rather than leaving a blank.
 */
export function resolveMedia(url: string): Promise<string> {
  if (fetcher == null || !url.startsWith("/media/")) return Promise.resolve(url);
  let hit = resolved.get(url);
  if (hit == null) {
    hit = fetcher(url).catch(() => url);
    resolved.set(url, hit);
  }
  return hit;
}
