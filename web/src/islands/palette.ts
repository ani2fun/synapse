import * as log from "../lib/log";
// The ⌘K palette (oracle: client/src/search/{state,view}/mod.rs — `SearchStore` +
// `SearchPalette`/`SearchButton`), a singleton modal mounted once per page from `Base.astro` so
// it exists on EVERY page (the e2e palette spec opens it from `/`). Vanilla TS: there is no
// component framework instance for this to hydrate into, only the `.header__search` button
// `Header.astro` has shipped inert since A03.
//
// Data loads LAZILY on first open — `fetchIndex()` + `blogList()` — and is cached for the rest
// of the page's life; a failed index load degrades to an empty result set exactly like the
// Rust memo (`AsyncResult::Loading | Failed` → `Vec::new()`).

import { fetchIndex, blogList } from "../lib/api/client";
import { entries as flattenEntries, search as rankSearch } from "../lib/search";
import type { SearchEntry, SearchKind } from "../lib/search";
import { pageUrl } from "../lib/routes";

let cachedEntries: SearchEntry[] | null = null;
let loadPromise: Promise<SearchEntry[]> | null = null;

/** Fetch once, keep forever (a page has one library and one blog list). Both calls degrade
 *  independently — a blog failure still leaves the lessons/books searchable, matching the
 *  Rust memo's `match blog.list().get() { Loaded(posts) => …, _ => logic::entries(&index, &[]) }`. */
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
  }

  toggle(): void {
    if (this.isOpen) this.close();
    else this.open();
  }

  open(): void {
    this.query = "";
    this.selected = 0;
    this.isOpen = true;
    this.mount();
    void this.refresh();
  }

  close(): void {
    this.isOpen = false;
    this.unmount();
  }

  private async refresh(): Promise<void> {
    const all = await loadEntries();
    // The palette may have closed (or re-opened and re-queried) while this was in flight.
    if (!this.isOpen) return;
    this.results = rankSearch(this.query, all);
    this.selected = clamp(this.selected, this.results.length);
    this.renderResults();
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
    input.placeholder = "Search lessons, books, posts…";
    input.addEventListener("input", () => {
      this.query = input.value;
      this.selected = 0;
      this.results = rankSearch(this.query, cachedEntries ?? []);
      this.selected = clamp(this.selected, this.results.length);
      this.renderResults();
    });

    const resultsEl = document.createElement("ul");
    resultsEl.className = "cmdk__results";

    panel.append(input, resultsEl);
    scrim.append(panel);
    document.body.append(scrim);

    this.scrim = scrim;
    this.input = input;
    this.resultsEl = resultsEl;
    input.focus();
  }

  private unmount(): void {
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
          log.info(`palette → ${pageUrl(entry.page)}`);
          window.location.href = pageUrl(entry.page);
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
    a.href = pageUrl(entry.page);
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

    a.append(kind, text);
    li.append(a);
    return li;
  }
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
