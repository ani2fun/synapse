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
// on this book's reader list (403), or the lesson. A `kind: problem` lesson renders its description
// and editorial as prose — no workbench, no Submit — and says so.
//
// Loaded ONLY in the page's private mode, so the ordinary lesson's eager budget does not move.
import { ApiFailure, fetchIndex, lesson as fetchLesson } from "../../lib/api/client";
import type { LessonPayload } from "../../lib/api/client";
import type { components } from "../../lib/api/schema.gen";
import { bookOf, problemContentSplit } from "../../lib/catalog/tree";
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

function renderSidebar(book: Book, current: string): void {
  const aside = root?.querySelector<HTMLElement>("[data-private-sidebar]");
  if (!aside) return;
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
  const last = path.split("/").pop() ?? path;
  return last
    .split("-")
    .map((w) => (w === "" ? "" : w[0].toUpperCase() + w.slice(1)))
    .join(" ");
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

async function render(payload: LessonPayload, segments: string[]): Promise<void> {
  const title = root?.querySelector<HTMLElement>("[data-private-title]");
  if (title) title.textContent = payload.frontmatter.title;
  const lede = root?.querySelector<HTMLElement>("[data-private-lede]");
  if (lede) {
    lede.textContent = payload.frontmatter.summary ?? "";
    lede.hidden = !payload.frontmatter.summary;
  }
  document.title = `${payload.book.title} · ${payload.frontmatter.title} — Synapse`;

  // The problem shape carries no workbench here: the judge, the tests and the submission
  // history are wired from server-rendered state this page never had. Description and editorial
  // read as prose, and the page says what it left out.
  let markdown = payload.raw;
  let note = "";
  if (payload.frontmatter.kind === "problem") {
    const [description, inlineEditorial] = problemContentSplit(payload.raw);
    const editorial = inlineEditorial.trim() !== "" ? inlineEditorial : (payload.editorial ?? "");
    markdown = editorial.trim() === "" ? description : `${description}\n\n---\n\n## Editorial\n\n${editorial}`;
    note = "This is a problem lesson in a private book: it reads as prose here, without the workbench or Submit.";
  }

  const { renderPreview, hydratePreview } = await import("../authoring/preview");
  const { bodyHtml } = await renderPreview(markdown);
  const body = root?.querySelector<HTMLElement>("[data-private-body]");
  if (!body) return;
  body.innerHTML = bodyHtml;
  await hydratePreview(body);
  status(note);
  renderPager(payload);

  try {
    const index = await fetchIndex();
    const book = bookOf(index, segments);
    if (book) renderSidebar(book, segments.join("/"));
  } catch (error) {
    log.debug(`private lesson: no sidebar (${error instanceof Error ? error.message : String(error)})`);
  }
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
