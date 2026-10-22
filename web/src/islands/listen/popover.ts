// ──────────────────────────────────────────────────────────────────
// THE SELECTION POPOVER — "Listen" and "From here" beside selected prose
// ──────────────────────────────────────────────────────────────────
// One popover, two modes. LIVE (a reader on the listen list) wires both buttons to the player.
// GATED (anyone else, where a speech engine exists) shows the same two buttons, plainly not
// actionable, under the tooltip that says how to ask — the "Suggest an edit" grammar: a reader
// should learn the feature exists rather than meet nothing at all.
//
// `aria-disabled`, never `disabled`: a disabled control swallows hover in some browsers, and the
// hover is what carries the explanation.

import { LISTEN_ACCESS_TEXT } from "../../lib/contact";

export interface PopoverActions {
  /** Read the selection. Absent in gated mode. */
  listen?: (range: Range) => void;
  /** Read from the selection's block to the end. Absent in gated mode. */
  fromHere?: (range: Range) => void;
}

function pill(text: string, label: string, gated: boolean): HTMLButtonElement {
  const el = document.createElement("button");
  el.type = "button";
  el.className = gated ? "listen-pop__btn listen-pop__btn--gated" : "listen-pop__btn";
  el.textContent = text;
  el.setAttribute("aria-label", label);
  if (gated) el.setAttribute("aria-disabled", "true");
  return el;
}

/** Mount the popover over `body`; returns a function that removes it and its listener. */
export function mountPopover(body: HTMLElement, actions: PopoverActions): () => void {
  const gated = !actions.listen;
  const pop = document.createElement("div");
  pop.className = gated ? "listen-pop listen-pop--gated" : "listen-pop";
  pop.hidden = true;
  pop.dataset.listenPop = gated ? "gated" : "live";
  if (gated) pop.dataset.tip = LISTEN_ACCESS_TEXT;
  const listenSel = pill("▶ Listen", "Listen to the selection", gated);
  const fromHere = pill("From here", "Listen from here to the end", gated);
  pop.append(listenSel, fromHere);
  document.body.append(pop);

  let range: Range | null = null;
  // `pointerdown` on the popover would otherwise collapse the selection before the click lands.
  pop.addEventListener("pointerdown", (event) => event.preventDefault());
  listenSel.addEventListener("click", () => {
    if (range && actions.listen) {
      actions.listen(range);
      pop.hidden = true;
    }
  });
  fromHere.addEventListener("click", () => {
    if (range && actions.fromHere) {
      actions.fromHere(range);
      pop.hidden = true;
    }
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
  const onSelection = (): void => {
    window.clearTimeout(timer);
    // Settle first: a drag fires dozens of these, and the popover should land once.
    timer = window.setTimeout(place, 200);
  };
  document.addEventListener("selectionchange", onSelection);

  return () => {
    window.clearTimeout(timer);
    document.removeEventListener("selectionchange", onSelection);
    pop.remove();
  };
}
