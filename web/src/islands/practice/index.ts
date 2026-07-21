/**
 * Practice-widget hydration: find every `div.practice-problem` the pipeline planted, decode its
 * attribute payloads into a `PracticeSpec`, and render a live Preact `PracticeProblem` two-pane
 * card into it. Auto-hydrates on import — the lesson page's script is the trigger; a problem page
 * owns its own workbench hydration and never imports this, but the guard is the belt to that
 * suspenders (an embedded widget is `.pwb--embedded`, NOT `.pwb[data-problem]`, so the guard
 * fires only on the problem PAGE).
 */
import { render, h } from "preact";

import { decodePractice } from "../../lib/execution/practice";
import { PracticeProblem } from "./PracticeProblem";
import * as log from "../../lib/log";

function decodedAttr(element: Element, name: string): string | null {
  const raw = element.getAttribute(name);
  if (raw == null) return null;
  try {
    return decodeURIComponent(raw);
  } catch {
    return null;
  }
}

function lessonPathFromUrl(): string[] {
  const path = window.location.pathname;
  if (!path.startsWith("/synapse/")) return [];
  return path
    .slice("/synapse/".length)
    .split("/")
    .filter((segment) => segment !== "");
}

/** Walk back to the nearest heading; take the text after "Practice:", else the heading, else "Your
 *  Turn". */
function practiceTitle(element: Element): string {
  let cur = element.previousElementSibling;
  while (cur) {
    if (/^H[1-6]$/.test(cur.tagName)) {
      const text = cur.textContent ?? "";
      const at = text.indexOf("Practice:");
      const title = at >= 0 ? text.slice(at + "Practice:".length).trim() : text.trim();
      return title === "" ? "Your Turn" : title;
    }
    cur = cur.previousElementSibling;
  }
  return "Your Turn";
}

export function hydratePractices(root: ParentNode): number {
  const lessonPath = lessonPathFromUrl();
  let count = 0;
  for (const element of root.querySelectorAll("div.practice-problem")) {
    const problem = decodedAttr(element, "data-problem");
    const variants = decodedAttr(element, "data-variants");
    if (problem == null || variants == null) continue;
    const spec = decodePractice(problem, variants, decodedAttr(element, "data-spec"), decodedAttr(element, "data-editorials"));
    if (!spec) continue;
    const title = practiceTitle(element);
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(PracticeProblem, { spec, title, lessonPath }), host);
    count += 1;
  }
  if (count > 0) log.info(`hydrated ${count} practice widget(s)`);
  return count;
}

// Auto-hydrate on import — see the module doc for the problem-page guard.
if (!document.querySelector(".pwb[data-problem]")) {
  hydratePractices(document);
}
