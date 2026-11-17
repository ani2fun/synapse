/**
 * The exercise set: a `<div class="exercise-set">` wrapping an exercise list and the
 * `<details>` that holds its answers, both authored as plain markdown. Hydration pairs item N
 * with answer N and enhances the list IN PLACE:
 *
 * - a gap-fill item (`______` in the text) gets an input per blank, Check and Show;
 * - any other item gets Show, which opens its own answer right under it;
 * - the `<details>` is replaced by one bar: score, Check all, Reveal all, Reset.
 *
 * Vanilla TS rather than Preact on purpose: the authored markup (bold, links, line breaks)
 * stays the DOM the reader sees, and the state is a handful of per-item flags. Anything the
 * hydrator does not understand (no list, item counts that differ) is left exactly as authored,
 * so a bad block degrades to today's page, never to a broken one.
 */
import * as log from "../../lib/log";
import { BLANK, expectedFills, grade, type Grade } from "./exerciseMatch";

const LIST = ":scope > ol, :scope > ul";

interface Item {
  li: HTMLElement;
  inputs: HTMLInputElement[];
  fills: string[];
  answer: HTMLElement;
  show: HTMLButtonElement;
  right: boolean;
}

function button(label: string, className: string): HTMLButtonElement {
  const el = document.createElement("button");
  el.type = "button";
  el.className = className;
  el.textContent = label;
  return el;
}

function items(list: Element): HTMLElement[] {
  return Array.from(list.querySelectorAll<HTMLElement>(":scope > li"));
}

/** The answer item's first line — text and bold runs up to its first `<br>`. The translation
 *  after the break is not part of what the blanks expect. */
function answerLine(li: HTMLElement): { text: string; bold: string[] } {
  let text = "";
  const bold: string[] = [];
  const walker = document.createTreeWalker(li, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (node instanceof HTMLBRElement) break;
    if (node instanceof Element && (node.tagName === "STRONG" || node.tagName === "B")) {
      bold.push(node.textContent ?? "");
    } else if (node.nodeType === Node.TEXT_NODE) {
      text += node.textContent ?? "";
    }
  }
  return { text, bold };
}

/** Replace every blank run in the item's text with an input, in document order. */
function plantInputs(li: HTMLElement, fills: string[], index: number): HTMLInputElement[] {
  const texts: Text[] = [];
  const walker = document.createTreeWalker(li, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (BLANK.test(node.textContent ?? "")) texts.push(node as Text);
    BLANK.lastIndex = 0;
  }
  // Mutate only when the blanks in the DOM are exactly the blanks the fills were worked out for.
  const total = texts.reduce((n, node) => n + ((node.textContent ?? "").match(BLANK)?.length ?? 0), 0);
  if (total !== fills.length) return [];
  const inputs: HTMLInputElement[] = [];
  for (const node of texts) {
    const parts = (node.textContent ?? "").split(BLANK);
    const fragment = document.createDocumentFragment();
    parts.forEach((part, i) => {
      if (i > 0) {
        const input = document.createElement("input");
        input.type = "text";
        input.className = "exercise__blank";
        input.autocomplete = "off";
        input.spellcheck = false;
        input.setAttribute("autocapitalize", "off");
        input.setAttribute("aria-label", `Item ${index + 1}, blank ${inputs.length + 1}`);
        input.addEventListener("input", () => {
          input.style.width = `${Math.max(9, input.value.length + 2)}ch`;
          input.removeAttribute("data-grade");
        });
        inputs.push(input);
        fragment.append(input);
      }
      if (part !== "") fragment.append(part);
    });
    node.replaceWith(fragment);
  }
  return inputs;
}

function hydrateSet(set: HTMLElement): boolean {
  const questions = set.querySelector(LIST);
  const details = set.querySelector<HTMLDetailsElement>(":scope > details");
  const answers = details?.querySelector(LIST);
  if (!questions || !details || !answers) {
    log.warn("exercise set skipped: needs a list and a <details> with an answer list");
    return false;
  }
  const qs = items(questions);
  const as = items(answers);
  if (qs.length === 0 || qs.length !== as.length) {
    log.warn(`exercise set skipped: ${qs.length} items but ${as.length} answers`);
    return false;
  }

  const state: Item[] = qs.map((li, i) => {
    const line = answerLine(as[i]);
    const fills = expectedFills(li.textContent ?? "", line.text, line.bold) ?? [];
    const inputs = fills.length > 0 ? plantInputs(li, fills, i) : [];

    const answer = document.createElement("div");
    answer.className = "exercise__answer";
    answer.hidden = true;
    answer.setAttribute("aria-live", "polite");
    answer.append(...Array.from(as[i].cloneNode(true).childNodes));

    const actions = document.createElement("span");
    actions.className = "exercise__actions not-prose";
    const show = button("Show", "exercise__btn");
    const item: Item = {
      li,
      inputs,
      fills: inputs.length > 0 ? fills : [],
      answer,
      show,
      right: false,
    };
    if (inputs.length > 0) {
      const check = button("Check", "exercise__btn exercise__btn--primary");
      check.addEventListener("click", () => checkItem(item));
      for (const input of inputs) {
        input.addEventListener("keydown", (event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            checkItem(item);
          }
        });
      }
      actions.append(check);
    }
    // `hidden` is typed `boolean | "until-found"`; this widget only ever sets the boolean.
    show.addEventListener("click", () => setOpen(item, item.answer.hidden !== false));
    actions.append(show);
    li.append(actions, answer);
    return item;
  });

  const gapItems = state.filter((item) => item.inputs.length > 0);
  const bar = document.createElement("div");
  bar.className = "exercise__bar not-prose";
  const score = document.createElement("span");
  score.className = "exercise__score";
  const revealAll = button("Reveal all", "exercise__btn");
  const reset = button("Reset", "exercise__btn");

  const updateScore = () => {
    const right = gapItems.filter((item) => item.right).length;
    score.textContent = `Score ${right} / ${gapItems.length}`;
    revealAll.textContent = state.every((item) => !item.answer.hidden) ? "Hide all" : "Reveal all";
  };

  function setOpen(item: Item, open: boolean): void {
    item.answer.hidden = !open;
    item.show.textContent = open ? "Hide" : "Show";
    item.show.setAttribute("aria-expanded", String(open));
    updateScore();
  }

  function checkItem(item: Item): void {
    const grades: Grade[] = item.inputs.map((input, i) => grade(input.value, item.fills[i]));
    item.inputs.forEach((input, i) => {
      input.dataset.grade = grades[i];
      input.setAttribute("aria-invalid", String(grades[i] !== "right"));
    });
    item.right = grades.every((g) => g === "right");
    item.li.dataset.grade = item.right ? "right" : grades.includes("wrong") ? "wrong" : "accent";
    setOpen(item, true);
  }

  if (gapItems.length > 0) {
    const checkAll = button("Check all", "exercise__btn exercise__btn--primary");
    checkAll.addEventListener("click", () => gapItems.forEach(checkItem));
    bar.append(score, checkAll);
  }
  revealAll.addEventListener("click", () => {
    const open = state.some((item) => item.answer.hidden);
    state.forEach((item) => setOpen(item, open));
  });
  reset.addEventListener("click", () => {
    for (const item of state) {
      for (const input of item.inputs) {
        input.value = "";
        input.style.width = "";
        input.removeAttribute("data-grade");
        input.removeAttribute("aria-invalid");
      }
      item.right = false;
      item.li.removeAttribute("data-grade");
      setOpen(item, false);
    }
  });
  bar.append(revealAll, reset);

  details.replaceWith(bar);
  set.dataset.hydrated = "1";
  updateScore();
  return true;
}

/** Enhance every exercise set under `root`; returns how many were hydrated. Idempotent. */
export function hydrateExercises(root: ParentNode): number {
  let count = 0;
  for (const set of root.querySelectorAll<HTMLElement>("div.exercise-set:not([data-hydrated])")) {
    if (hydrateSet(set)) count += 1;
  }
  return count;
}
