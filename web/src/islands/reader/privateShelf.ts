// The "Your private books" shelf on the sidebar's browse face. The rail is server-rendered from
// the anonymous index, so this adds the reader's own books once the session is known: it fetches
// the index WITH the bearer and slots the shelf under the public "All courses" list in every
// browse face in the document — the desktop aside and the problem page's hidden drawer source.
// The mobile drawer clones the aside when it opens, so it carries the shelf with no work here.
//
// Idempotent: every pass removes the shelf it finds first, so a sign-in, a sign-out or a second
// caller (a private lesson renders its own rail, then mounts the shelf into it) never stacks two.
// Loaded on demand by `islands/reader` for a signed-in reader, so anonymous pages pay nothing.
import { fetchIndex } from "../../lib/api/client";
import type { SynapseIndex } from "../../lib/api/client";
import * as log from "../../lib/log";
import { isAuthed } from "../workbench/contracts";
import { privateShelfBooks, privateShelfHtml, SHELF_ATTR } from "./shelf";

/** Put the shelf for `index` into every browse face under `root`, replacing any shelf there. */
export function mountPrivateShelf(index: SynapseIndex | null, root: ParentNode = document): number {
  const books = index ? privateShelfBooks(index) : [];
  let mounted = 0;
  for (const browse of Array.from(root.querySelectorAll<HTMLElement>(".reader-sidebar__browse"))) {
    browse.querySelectorAll(`[${SHELF_ATTR}]`).forEach((node) => node.remove());
    const publicList = browse.querySelector<HTMLElement>(`.reader-sidebar__browse-list:not([${SHELF_ATTR}])`);
    const html = privateShelfHtml(books, browse.closest<HTMLElement>("[data-book-slug]")?.dataset.bookSlug ?? null);
    if (!publicList || html === "") continue;
    publicList.insertAdjacentHTML("afterend", html);
    mounted += 1;
  }
  return mounted;
}

/** Signed in: fetch the reader's index and mount its shelf. Signed out: take any shelf down. */
export async function refreshPrivateShelf(): Promise<void> {
  if (!document.querySelector(".reader-sidebar__browse")) return;
  if (!isAuthed()) {
    mountPrivateShelf(null);
    return;
  }
  try {
    const index = await fetchIndex();
    const mounted = mountPrivateShelf(index);
    log.info(`reader: private shelf → ${privateShelfBooks(index).length} book(s) in ${mounted} sidebar(s)`);
  } catch (error) {
    log.debug(`reader: private shelf skipped (${error instanceof Error ? error.message : String(error)})`);
  }
}
