// ──────────────────────────────────────────────────────────────────
// LISTEN AFFORDANCES — a header button and a popover over a selection
// ──────────────────────────────────────────────────────────────────
// Loaded only for a reader the server says may listen (see `./index`), so nothing here ships to
// anyone else. Two ways in:
//
//   · "Listen" in the lesson header — the whole lesson from the top;
//   · select some prose and a small popover offers "Listen" (just the selection) and
//     "From here" (from the selection's paragraph to the end of the lesson).

import type { ListenConfig } from "../../lib/api/client";
import * as log from "../../lib/log";
import { blockAt, proseFrom, proseInRange } from "./extract";
import { createPlayer } from "./player";
import { splitIntoUnits } from "./units";

const BODY = "article.lesson-body";

function pill(text: string, label: string): HTMLButtonElement {
  const el = document.createElement("button");
  el.type = "button";
  el.className = "listen-pop__btn";
  el.textContent = text;
  el.setAttribute("aria-label", label);
  return el;
}

export function mount(config: ListenConfig): void {
  const body = document.querySelector<HTMLElement>(BODY);
  if (!body) return;
  const player = createPlayer(config);
  const title = document.querySelector("h1")?.textContent?.trim() ?? document.title;
  const listen = (blocks: string[], label: string): void => {
    void player.play(splitIntoUnits(blocks, config.maxUnitChars), label);
  };

  // ── the header button: the whole lesson ──
  const header = document.createElement("button");
  header.type = "button";
  header.className = "listen-header";
  header.dataset.listenHeader = "";
  header.textContent = "▶ Listen";
  header.setAttribute("aria-label", "Listen to this lesson");
  header.addEventListener("click", () => {
    const first = body.firstElementChild;
    if (first) listen(proseFrom(body, first), title);
  });
  const anchor = document.querySelector("[data-edit-tip]") ?? document.querySelector(".reader-prose__title");
  anchor?.after(header);

  // ── the selection popover ──
  const pop = document.createElement("div");
  pop.className = "listen-pop";
  pop.hidden = true;
  pop.dataset.listenPop = "";
  const listenSel = pill("▶ Listen", "Listen to the selection");
  const fromHere = pill("From here", "Listen from here to the end");
  pop.append(listenSel, fromHere);
  document.body.append(pop);

  let range: Range | null = null;
  // `pointerdown` on the popover would otherwise collapse the selection before the click lands.
  pop.addEventListener("pointerdown", (event) => event.preventDefault());
  listenSel.addEventListener("click", () => {
    if (range) listen(proseInRange(body, range), title);
    pop.hidden = true;
  });
  fromHere.addEventListener("click", () => {
    const start = range ? blockAt(body, range.startContainer) : null;
    if (start) listen(proseFrom(body, start), title);
    pop.hidden = true;
  });

  const place = (): void => {
    const selection = document.getSelection();
    const live = selection && selection.rangeCount > 0 ? selection.getRangeAt(0) : null;
    if (!live || live.collapsed || !body.contains(live.commonAncestorContainer)) {
      pop.hidden = true;
      range = null;
      return;
    }
    range = live.cloneRange();
    const rect = live.getBoundingClientRect();
    pop.hidden = false;
    pop.style.top = `${window.scrollY + rect.bottom + 8}px`;
    pop.style.left = `${Math.max(8, window.scrollX + rect.left)}px`;
  };
  let timer: number | undefined;
  document.addEventListener("selectionchange", () => {
    window.clearTimeout(timer);
    // Settle first: a drag fires dozens of these, and the popover should land once.
    timer = window.setTimeout(place, 200);
  });

  log.info("listen: affordances mounted");
}
