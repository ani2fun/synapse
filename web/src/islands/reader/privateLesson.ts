// A PRIVATE book's lesson, rendered in the browser.
//
// The lesson page is server-rendered from an anonymous fetch, and a private book refuses that
// fetch — the Keycloak token exists only here, in the browser. So the page ships a shell (the
// reader chrome, the title humanised from the URL, an empty body) and this island fills it once
// the session has settled: it asks the API again WITH the bearer, and renders the payload through
// the exact pipeline and hydrators the authoring preview already runs client-side
// (`renderPreview` + `hydratePreview`). What it cannot do is prerender d2 figures — those fall to
// the client renderer, as they do whenever the sidecar is absent.
//
// Three outcomes, each said plainly in the shell's status line: sign in (anonymous), you are not
// on this book's reader list (403), or the lesson. A `kind: problem` lesson gets the problem PAGE:
// the same frame the server renders for a public one (`lib/catalog/problemFrame`), built here from
// the payload, and then `islands/problem` hydrates it — workbench, tabs, canvas, submissions.
//
// Loaded ONLY in the page's private mode, so the ordinary lesson's eager budget does not move.
import { ApiFailure, bearerHeaders, fetchIndex, lesson as fetchLesson } from "../../lib/api/client";
import type { LessonPayload } from "../../lib/api/client";
import type { components } from "../../lib/api/schema.gen";
import { DEFAULT_LEFT_PCT } from "../../lib/catalog/pane";
import { humanize, problemFrame } from "../../lib/catalog/problemFrame";
import { bookOf, chapterProblems, problemContentSplit } from "../../lib/catalog/tree";
import * as log from "../../lib/log";
import { boot, getState, signIn, subscribe } from "../auth/store";

type Book = components["schemas"]["BookDto"];
type BookEntry = components["schemas"]["BookEntryDto"];

const root = document.querySelector<HTMLElement>("[data-private-lesson]");

function status(text: string, action?: { label: string; run: () => void }): void {
  const line = root?.querySelector<HTMLElement>("[data-private-status]");
  if (!line) return;
  line.textContent = text;
  if (action) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "account-page__signin private-lesson__signin";
    button.textContent = action.label;
    button.addEventListener("click", action.run);
    line.append(" ", button);
  }
}

function escape(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** The sidebar tree the Astro `SidebarTree` renders for a public book, rebuilt from the index
 *  the reader was admitted to. Same classes, so the reader chrome styles it the same. */
function tree(entries: BookEntry[], prefix: string[], current: string): string {
  return entries
    .map((entry) => {
      if (entry.kind === "lesson") {
        const full = [...prefix, entry.slug].join("/");
        const active = current === full ? " reader-sidebar__link--active" : "";
        return `<li><a class="reader-sidebar__link${active}" href="/synapse/${full}">${escape(entry.title)}</a></li>`;
      }
      const segments = [...prefix, entry.slug];
      const only = entry.entries.length === 1 && entry.entries[0].kind === "lesson" ? entry.entries[0] : null;
      if (only && only.slug === entry.slug) {
        const leaf = [...segments, only.slug].join("/");
        const active = current === leaf ? " reader-sidebar__link--active" : "";
        return `<li><a class="reader-sidebar__link${active}" href="/synapse/${leaf}">${escape(only.title)}</a></li>`;
      }
      const open = `${current}/`.startsWith(`${segments.join("/")}/`) ? " open" : "";
      return (
        `<li><details class="reader-sidebar__section"${open}><summary class="reader-sidebar__summary">` +
        `<span class="reader-sidebar__name">${escape(entry.title)}</span></summary>` +
        `<ul class="reader-sidebar__children">${tree(entry.entries, segments, current)}</ul></details></li>`
      );
    })
    .join("");
}

function renderSidebar(aside: HTMLElement, book: Book, current: string): void {
  aside.innerHTML =
    `<div class="reader-sidebar__inner" data-view="book"><div class="reader-sidebar__book-view">` +
    `<div class="reader-sidebar__toprow"><a class="reader-sidebar__home" href="/">← Library</a></div>` +
    `<div class="reader-sidebar__book-head"><div class="reader-sidebar__book-txt">` +
    `<div class="reader-sidebar__eyebrow">🔒 Private book</div>` +
    `<div class="reader-sidebar__title">${escape(book.title)}</div></div></div>` +
    (book.description ? `<div class="reader-sidebar__desc">${escape(book.description)}</div>` : "") +
    `<ul class="reader-sidebar__tree">${tree(book.entries, [...book.categoryPath, book.slug], current)}</ul>` +
    `</div></div>`;
}

/** `two-sum` → `Two Sum`, the pager's own rule. */
function humanise(path: string): string {
  return humanize(path.split("/").pop() ?? path);
}

function renderPager(payload: LessonPayload): void {
  const nav = root?.querySelector<HTMLElement>("[data-private-pager]");
  if (!nav) return;
  const card = (target: string | null | undefined, label: string, next: boolean) => {
    const cls = next ? "reader-pager__card reader-pager__card--next" : "reader-pager__card";
    if (!target) return `<span class="${cls} reader-pager__card--empty" aria-hidden="true"></span>`;
    return (
      `<a class="${cls}" href="/synapse/${target}"><span class="reader-pager__label">${label}</span>` +
      `<span class="reader-pager__title">${escape(humanise(target))}</span></a>`
    );
  };
  nav.innerHTML = card(payload.prev, "Previous", false) + card(payload.next, "Next", true);
}

/**
 * A private source's `/media/…` files are gated like its prose, and an `<img>` (or a `<video>`,
 * `<audio>`, `<source>`) carries no bearer. So every media reference in the rendered body is
 * fetched here WITH the bearer and swapped for a blob URL the browser can show. A file that is
 * refused or missing keeps its original `src`, so the broken-image mark says what happened
 * rather than a blank.
 */
async function attachPrivateMedia(body: HTMLElement): Promise<void> {
  const nodes = body.querySelectorAll<HTMLImageElement | HTMLMediaElement | HTMLSourceElement>(
    'img[src^="/media/"], video[src^="/media/"], audio[src^="/media/"], source[src^="/media/"]',
  );
  let swapped = 0;
  await Promise.all(
    Array.from(nodes).map(async (node) => {
      const src = node.getAttribute("src");
      if (!src) return;
      try {
        const response = await fetch(src, { headers: bearerHeaders() });
        if (!response.ok) return;
        node.src = URL.createObjectURL(await response.blob());
        swapped += 1;
      } catch (error) {
        log.debug(`private media skipped: ${src} (${error instanceof Error ? error.message : String(error)})`);
      }
    }),
  );
  // A `<source>` swap only takes effect once its parent reloads.
  for (const media of body.querySelectorAll<HTMLMediaElement>("video, audio")) media.load();
  if (swapped > 0) log.debug(`private lesson: ${swapped} media file(s) fetched with the bearer`);
}

/** The book this lesson sits in, from the index the reader was admitted to — or null when the
 *  index cannot be read; the page still renders, without a rail or a counter. */
async function admittedBook(segments: string[]): Promise<Book | null> {
  try {
    return bookOf(await fetchIndex(), segments);
  } catch (error) {
    log.debug(`private lesson: no index (${error instanceof Error ? error.message : String(error)})`);
    return null;
  }
}

/**
 * A problem lesson becomes the problem PAGE: the shell is replaced by the frame the server
 * renders for a public problem, built from the payload — description rendered through the
 * reader's pipeline, the raw editorial for the stepper, the sample suite for the workbench, the
 * chapter counter from the admitted index — and `islands/problem` then hydrates it exactly as it
 * would after a server render. The judge reads the suite from the same mounted source, so Submit
 * works; the Contents drawer clones the hidden sidebar source the public page also carries.
 */
async function renderProblem(payload: LessonPayload, segments: string[]): Promise<void> {
  const main = root?.closest<HTMLElement>("main.shell-main");
  if (!main) return;
  const [descriptionMd, inlineEditorial] = problemContentSplit(payload.raw);
  const editorialMd = inlineEditorial.trim() !== "" ? inlineEditorial : (payload.editorial ?? "");
  const { renderLesson } = await import("../../lib/markdown/render");
  const descriptionHtml = await renderLesson(descriptionMd);
  const book = await admittedBook(segments);
  const current = segments.join("/");

  main.innerHTML = problemFrame({
    title: payload.frontmatter.title,
    lede: payload.frontmatter.summary ?? null,
    bookName: humanize(payload.book.slug),
    difficulty: payload.frontmatter.difficulty ?? null,
    descriptionHtml,
    editorialMd,
    tests: payload.tests ?? null,
    counter: book ? chapterProblems(book, current) : null,
    prev: payload.prev ?? null,
    next: payload.next ?? null,
    leftPct: DEFAULT_LEFT_PCT,
  });
  if (book) {
    const src = document.createElement("div");
    src.className = "pwb-sidebar-src";
    src.hidden = true;
    const aside = document.createElement("aside");
    aside.className = "reader-sidebar";
    src.append(aside);
    main.append(src);
    renderSidebar(aside, book, current);
  }
  const description = main.querySelector<HTMLElement>(".pwb-description");
  if (description) await attachPrivateMedia(description);
  document.title = `${payload.book.title} · ${payload.frontmatter.title} — Synapse`;
  // The problem island hydrates `.pwb[data-problem]` on import: the DOM is ready by now, so it
  // runs at once. Loaded here, not by the page script, because the page could not know the shape
  // of a lesson it was refused.
  await import("../problem");
  log.info(`private problem rendered client-side: /synapse/${current}`);
}

async function render(payload: LessonPayload, segments: string[]): Promise<void> {
  if (payload.frontmatter.kind === "problem") return renderProblem(payload, segments);

  const title = root?.querySelector<HTMLElement>("[data-private-title]");
  if (title) title.textContent = payload.frontmatter.title;
  const lede = root?.querySelector<HTMLElement>("[data-private-lede]");
  if (lede) {
    lede.textContent = payload.frontmatter.summary ?? "";
    lede.hidden = !payload.frontmatter.summary;
  }
  document.title = `${payload.book.title} · ${payload.frontmatter.title} — Synapse`;

  const { renderPreview, hydratePreview } = await import("../authoring/preview");
  const { bodyHtml } = await renderPreview(payload.raw);
  const body = root?.querySelector<HTMLElement>("[data-private-body]");
  if (!body) return;
  body.innerHTML = bodyHtml;
  await attachPrivateMedia(body);
  await hydratePreview(body);
  status("");
  renderPager(payload);

  const book = await admittedBook(segments);
  const aside = root?.querySelector<HTMLElement>("[data-private-sidebar]");
  if (book && aside) renderSidebar(aside, book, segments.join("/"));
  log.info(`private lesson rendered client-side: /synapse/${segments.join("/")}`);
}

async function load(segments: string[]): Promise<void> {
  status("Checking your access…");
  try {
    await render(await fetchLesson(segments), segments);
  } catch (error) {
    if (error instanceof ApiFailure && error.status === 403) {
      status("This book is private, and you are not on its reader list. Ask the site admin to add you.");
    } else if (error instanceof ApiFailure && error.status === 401) {
      status("Sign in to read this book.", { label: "Sign in", run: () => signIn() });
    } else {
      status(`Could not load this lesson: ${error instanceof Error ? error.message : String(error)}`);
    }
    log.info(`private lesson refused: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function start(): void {
  if (!root) return;
  const segments = (root.dataset.privateLesson ?? "").split("/").filter((s) => s !== "");
  let handled = false;
  const settle = () => {
    const state = getState();
    if (state.kind === "loading" || handled) return;
    handled = true;
    unsubscribe();
    if (state.kind === "anonymous") {
      status("Sign in to read this book.", { label: "Sign in", run: () => signIn() });
      log.info("private lesson: anonymous, waiting for a sign-in");
      return;
    }
    void load(segments);
  };
  const unsubscribe = subscribe(settle);
  void boot().then(settle);
  settle();
}

start();
