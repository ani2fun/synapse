/**
 * Viewport-lazy Monaco.
 *
 * Two halves:
 *  - `watchNear`: an IntersectionObserver with a 600px vertical margin flips a callback when
 *    the block approaches the viewport. No-IO environments degrade to eagerly near.
 *  - the live-editor registry: at most MAX_LIVE_EDITORS Monaco instances exist at once; when a
 *    new one registers over the cap, the oldest FAR instance is evicted (its owner drops the
 *    editor and re-arms the lazy mount). Visible editors are never evicted — safe because all
 *    block state lives in the store, not Monaco, so a re-approach re-mounts losslessly.
 */

export const MAX_LIVE_EDITORS = 3;
const NEAR_MARGIN = "600px 0px 600px 0px";

export interface NearWatch {
  disconnect: () => void;
}

export function watchNear(node: Element, onNear: (near: boolean) => void): NearWatch | null {
  if (typeof IntersectionObserver === "undefined") {
    // No-IO environment: degrade to eager, exactly as the Rust does.
    onNear(true);
    return null;
  }
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) onNear(entry.isIntersecting);
    },
    { rootMargin: NEAR_MARGIN },
  );
  observer.observe(node);
  return { disconnect: () => observer.disconnect() };
}

interface LiveEntry {
  id: number;
  isNear: () => boolean;
  evict: () => void;
}

const live: LiveEntry[] = [];
let nextId = 0;

/** Register a mounted editor; may synchronously evict the oldest far one to stay under cap. */
export function register(isNear: () => boolean, evict: () => void): number {
  const id = nextId++;
  live.push({ id, isNear, evict });
  if (live.length > MAX_LIVE_EDITORS) {
    const victim = live.find((entry) => !entry.isNear());
    // All near → over cap but nobody evictable; visible editors are never torn down.
    if (victim) {
      deregister(victim.id);
      victim.evict();
    }
  }
  return id;
}

export function deregister(id: number): void {
  const at = live.findIndex((entry) => entry.id === id);
  if (at >= 0) live.splice(at, 1);
}
