import * as log from "../lib/log";
// The ⌘K palette, a singleton modal mounted once per page from `Base.astro` so it exists on
// EVERY page (the e2e palette spec opens it from `/`). Vanilla TS: there is no component
// framework instance for this to hydrate into, only the `.header__search` button `Header.astro`
// ships with.
//
// Data loads LAZILY on first open — `fetchIndex()` + `blogList()` — and is cached for the rest
// of the page's life; a failed index load degrades to an empty result set.
//
// Two searches run against every query. The local one is instant and sees titles and breadcrumbs
// only, because that is all the browsable index carries. The other asks the server, which sees
// the prose of every mounted repository — debounced, race-guarded by the query the reply echoes
// back, and STRICTLY additive: if it fails, or is slow, or the reader is offline, the palette is
// exactly what it was before it existed.
//
// A search can be SCOPED to one book: the lesson page's reader menu opens the palette through
// `OPEN_SEARCH` with the book in hand. The scope narrows both halves — the local ranking runs over
// that book's entries only, and the server is asked with `book=` so another book's better matches
// never fill the limit first. A chip names the scope; removing it (✕, or Backspace in an empty
// box) widens the search to everything, as ⌘K opens it.

import { fetchIndex, blogList, searchCatalog } from "../lib/api/client";
import type { SearchHit } from "../lib/api/client";
import { entries as flattenEntries, entryUrl, merge, search as rankSearch, withinBook } from "../lib/search";
import type { SearchEntry, SearchKind } from "../lib/search";
import { OPEN_SEARCH, type OpenSearch, type SearchScope } from "./workbench/contracts";

const PLACEHOLDER = "Search titles and prose…";
const SCOPED_PLACEHOLDER = "Search this book…";

/** Long enough that a burst of typing costs one request, short enough to feel instant. */
const DEBOUNCE_MS = 150;
/** Below this the server's prefix scan does not expand either, so there is nothing to ask for. */
const MIN_PROSE_QUERY = 2;

let cachedEntries: SearchEntry[] | null = null;
let loadPromise: Promise<SearchEntry[]> | null = null;

/** Fetch once, keep forever (a page has one library and one blog list). Both calls degrade
 *  independently — a blog failure still leaves the lessons/books searchable. */
async function loadEntries(): Promise<SearchEntry[]> {
  if (cachedEntries) return cachedEntries;
  if (loadPromise) return loadPromise;
  loadPromise = (async () => {
    let index;
    try {
      log.debug("palette: first open — loading the search index");
      index = await fetchIndex();
    } catch {
      return [];
    }
    let blog: Awaited<ReturnType<typeof blogList>> = [];
    try {
      blog = await blogList();
    } catch {
      blog = [];
    }
    const flat = flattenEntries(index, blog);
    cachedEntries = flat;
    return flat;
  })();
  return loadPromise;
}

function kindLabel(kind: SearchKind): string {
  switch (kind) {
    case "lesson":
      return "Lesson";
    // A problem's worked solution. It reads as its own kind because opening one spoils the
    // exercise, and a reader has to be able to tell that before clicking.
    case "editorial":
      return "Solution";
    case "book":
      return "Book";
    case "blog":
      return "Post";
  }
}

class Palette {
  private isOpen = false;
  private query = "";
  private selected = 0;
  private results: SearchEntry[] = [];
  /** The last full-text answer that was still current when it landed. Kept across keystrokes on
   *  purpose: clearing it on every one would collapse the list to "No matches." and repopulate it
   *  a debounce later, flickering through the whole of a word being typed. */
  private prose: SearchHit[] = [];
  private timer: ReturnType<typeof setTimeout> | null = null;
  /** The one book this search is confined to, or null for the whole library. */
  private scope: SearchScope | null = null;

  private scopeEl: HTMLDivElement | null = null;
  private scrim: HTMLDivElement | null = null;
  private input: HTMLInputElement | null = null;
  private resultsEl: HTMLUListElement | null = null;

  constructor() {
    window.addEventListener("keydown", (event) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        this.toggle();
      }
    });

    document.querySelectorAll<HTMLButtonElement>(".header__search").forEach((button) => {
      button.addEventListener("click", () => this.open());
    });

    window.addEventListener(OPEN_SEARCH, (event) => {
      const detail = (event as CustomEvent<OpenSearch | null>).detail;
      this.open(detail?.book ?? null);
    });
  }

  toggle(): void {
    if (this.isOpen) this.close();
    else this.open();
  }

  open(scope: SearchScope | null = null): void {
    // A second open with a different scope must rebuild the chip, not keep the first one's.
    if (this.isOpen) this.close();
    this.query = "";
    this.selected = 0;
    this.prose = [];
    this.scope = scope;
    this.isOpen = true;
    this.mount();
    log.info(`palette: open${scope ? ` — scoped to book ${scope.slug}` : ""}`);
    void this.refresh();
  }

  /** Widen a scoped search to the whole library, keeping what has been typed. */
  private clearScope(): void {
    if (!this.scope) return;
    log.debug(`palette: scope ${this.scope.slug} removed — searching every book`);
    this.scope = null;
    this.prose = [];
    this.scopeEl?.remove();
    this.scopeEl = null;
    if (this.input) this.input.placeholder = PLACEHOLDER;
    this.render();
    this.scheduleProse();
    this.input?.focus();
  }

  close(): void {
    this.isOpen = false;
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    this.unmount();
  }

  /** First open only: nothing can be ranked until the library index has arrived. */
  private async refresh(): Promise<void> {
    await loadEntries();
    // The palette may have closed (or re-opened and re-queried) while this was in flight.
    if (!this.isOpen) return;
    this.render();
  }

  /** Rank what is already in hand and paint. Synchronous by design — this runs on every
   *  keystroke, and waiting on the network to redraw a title match would be a regression. */
  private render(): void {
    const all = cachedEntries ?? [];
    const pool = this.scope ? withinBook(all, this.scope.slug) : all;
    const local = rankSearch(this.query, pool);
    this.results = merge(this.query, local, this.prose);
    this.selected = clamp(this.selected, this.results.length);
    this.renderResults();
  }

  private scheduleProse(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;

    const wanted = this.query.trim();
    if (wanted.length < MIN_PROSE_QUERY) {
      // Deleting back down to one letter must not leave the previous answer standing: those hits
      // were found for a word this query is no longer asking about.
      this.prose = [];
      return;
    }
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.fetchProse(wanted);
    }, DEBOUNCE_MS);
  }

  private async fetchProse(query: string): Promise<void> {
    const book = this.scope?.slug;
    let answer;
    try {
      answer = await searchCatalog(query, book);
    } catch {
      // An enhancement that failed. Say nothing to the reader: the title matches on screen are
      // still the whole of what search did before this endpoint existed.
      log.debug(`palette: full-text search unavailable for "${query}" — titles only`);
      return;
    }
    // The ECHO decides, not the closure variable. Replies can land out of order, and this is the
    // only thing that tells an answer to what is typed now from an answer to what was typed two
    // keystrokes ago.
    // The scope echo decides the same way: a scoped reply landing after the chip was removed
    // would otherwise paint one book's hits into a search of every book.
    if (!this.isOpen || answer.query !== this.query.trim()) return;
    if ((answer.book ?? undefined) !== this.scope?.slug) return;
    log.debug(`palette: ${answer.results.length} prose hit(s) for "${query}"`);
    this.prose = answer.results;
    this.render();
  }

  private mount(): void {
    if (this.scrim) return;

    const scrim = document.createElement("div");
    scrim.className = "cmdk-scrim";
    scrim.addEventListener("click", (event) => {
      if (event.target === scrim) this.close();
    });

    const panel = document.createElement("div");
    panel.className = "cmdk";
    panel.addEventListener("keydown", (event) => this.handleKey(event));

    const input = document.createElement("input");
    input.className = "cmdk__input";
    input.placeholder = this.scope ? SCOPED_PLACEHOLDER : PLACEHOLDER;
    input.addEventListener("keydown", (event) => {
      if (event.key === "Backspace" && input.value === "" && this.scope) {
        event.preventDefault();
        this.clearScope();
      }
    });
    input.addEventListener("input", () => {
      this.query = input.value;
      this.selected = 0;
      this.render();
      this.scheduleProse();
    });

    const resultsEl = document.createElement("ul");
    resultsEl.className = "cmdk__results";

    if (this.scope) {
      this.scopeEl = this.scopeChip(this.scope);
      panel.append(this.scopeEl);
    }
    panel.append(input, resultsEl);
    scrim.append(panel);
    document.body.append(scrim);

    this.scrim = scrim;
    this.input = input;
    this.resultsEl = resultsEl;
    input.focus();
  }

  /** "In: <book> ✕" — what the search is confined to, and the one control that lifts it. */
  private scopeChip(scope: SearchScope): HTMLDivElement {
    const bar = document.createElement("div");
    bar.className = "cmdk__scope";
    const chip = document.createElement("span");
    chip.className = "cmdk__scope-chip";
    const label = document.createElement("span");
    label.className = "cmdk__scope-label";
    label.textContent = `In: ${scope.title}`;
    const clear = document.createElement("button");
    clear.type = "button";
    clear.className = "cmdk__scope-clear";
    clear.setAttribute("aria-label", `Search every book, not only ${scope.title}`);
    clear.textContent = "✕";
    clear.addEventListener("click", () => this.clearScope());
    chip.append(label, clear);
    bar.append(chip);
    return bar;
  }

  private unmount(): void {
    this.scopeEl = null;
    this.scrim?.remove();
    this.scrim = null;
    this.input = null;
    this.resultsEl = null;
  }

  private handleKey(event: KeyboardEvent): void {
    switch (event.key) {
      case "Escape":
        this.close();
        break;
      case "ArrowDown":
        event.preventDefault();
        this.selected = clamp(this.selected + 1, this.results.length);
        this.renderResults();
        break;
      case "ArrowUp":
        event.preventDefault();
        this.selected = clamp(this.selected - 1, this.results.length);
        this.renderResults();
        break;
      case "Enter": {
        event.preventDefault();
        const active = clamp(this.selected, this.results.length);
        const entry = this.results[active];
        if (entry) {
          this.close();
          const url = entryUrl(entry);
          log.info(`palette → ${url}`);
          window.location.href = url;
        }
        break;
      }
      default:
        break;
    }
  }

  private renderResults(): void {
    const el = this.resultsEl;
    if (!el) return;
    el.replaceChildren();

    if (this.results.length === 0) {
      const empty = document.createElement("li");
      empty.className = "cmdk__empty";
      empty.textContent = "No matches.";
      el.append(empty);
      return;
    }

    const active = clamp(this.selected, this.results.length);
    this.results.forEach((entry, i) => {
      el.append(this.resultRow(entry, i === active));
    });
  }

  private resultRow(entry: SearchEntry, active: boolean): HTMLLIElement {
    const li = document.createElement("li");
    const a = document.createElement("a");
    a.className = active ? "cmdk__result cmdk__result--active" : "cmdk__result";
    a.href = entryUrl(entry);
    a.addEventListener("click", () => this.close());

    const kind = document.createElement("span");
    kind.className = "cmdk__result-kind";
    kind.textContent = kindLabel(entry.kind);

    const text = document.createElement("span");
    text.className = "cmdk__result-text";

    const label = document.createElement("span");
    label.className = "cmdk__result-label";
    label.textContent = entry.label;
    text.append(label);

    if (entry.sublabel !== "") {
      const sub = document.createElement("span");
      sub.className = "cmdk__result-sub";
      sub.textContent = entry.sublabel;
      text.append(sub);
    }

    if (entry.snippet && entry.snippet.length > 0) {
      text.append(snippetLine(entry.snippet));
    }

    a.append(kind, text);
    li.append(a);
    return li;
  }
}

/**
 * The quote, built from text nodes and `<mark>` elements — never from markup.
 *
 * The server pre-splits the excerpt into matched and unmatched runs precisely so this can be a
 * DOM assembly rather than a parse: a query containing `<script>` becomes the characters
 * `<script>` on screen, with no sanitiser standing between it and the reader.
 */
function snippetLine(segments: { text: string; marked: boolean }[]): HTMLSpanElement {
  const line = document.createElement("span");
  line.className = "cmdk__result-snippet";
  for (const segment of segments) {
    if (segment.marked) {
      const mark = document.createElement("mark");
      mark.textContent = segment.text;
      line.append(mark);
    } else {
      line.append(document.createTextNode(segment.text));
    }
  }
  return line;
}

function clamp(i: number, count: number): number {
  if (count === 0) return 0;
  return Math.min(Math.max(i, 0), count - 1);
}

function init(): void {
  new Palette();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
