/**
 * Fence-group bars (port of client/src/execution/view/fence_group.rs, step-41 semantics),
 * vanilla TS — one piece of state per group (the active pane), plain DOM the rest of the way.
 *
 * Language tabs when adjacent plain fences differ, a lone ▶ pill otherwise; copy + "Try in
 * Editor" on the right. Try in Editor (A09) follows the SELECTED tab and only appears when the
 * sandbox speaks that language (`runnableFence`, shared with the language preference) — prose
 * fences (bash, json, yaml) still get the bar and the copy button, just not this one.
 */

const PLAY_SVG =
  '<svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"></path></svg>';
const COPY_SVG =
  '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="8" y="8" width="14" height="14" rx="2" ry="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg>';
const CHECK_SVG =
  '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>';

interface Pane {
  language: string;
  code: string;
  figure: Element;
}

function collectPanes(group: Element): Pane[] {
  const panes: Pane[] = [];
  for (const figure of group.querySelectorAll("figure[data-rehype-pretty-code-figure]")) {
    const pre = figure.querySelector("pre[data-language]");
    const language = pre?.getAttribute("data-language");
    const code = pre?.textContent ?? "";
    if (!pre || !language || code.trim() === "") continue;
    panes.push({ language, code, figure });
  }
  return panes;
}

/** Display name for the tab — mirrors logic::display_lang's title-casing of known aliases. */
import { displayLang } from "../../lib/execution/blocks";
import { runnableFence } from "../../lib/execution/language";
import { openCodebench } from "../widgets/Codebench";

function mountBar(group: Element): void {
  const bar = group.querySelector("div.fence-group__bar");
  const panes = collectPanes(group);
  if (!bar || panes.length === 0) return;

  let active = 0;
  const showActive = () => {
    panes.forEach((pane, index) =>
      pane.figure.classList.toggle("fence-group__pane--hidden", index !== active),
    );
  };

  const actions = document.createElement("div");
  actions.className = "fence-group__actions";
  const copy = document.createElement("button");
  copy.className = "fence-group__copy";
  copy.setAttribute("aria-label", "Copy code");
  copy.title = "Copy code";
  copy.innerHTML = COPY_SVG;
  copy.addEventListener("click", () => {
    void navigator.clipboard?.writeText(panes[active]!.code);
    copy.innerHTML = CHECK_SVG;
    copy.classList.add("fence-group__copy--done");
    setTimeout(() => {
      copy.innerHTML = COPY_SVG;
      copy.classList.remove("fence-group__copy--done");
    }, 1400);
  });
  actions.appendChild(copy);

  const tryBtn = document.createElement("button");
  tryBtn.className = "fence-group__try";
  tryBtn.title = "Open this code in the editor — run it, feed it input, make it yours";
  tryBtn.innerHTML = `${PLAY_SVG}<span>Try in Editor</span>`;
  tryBtn.addEventListener("click", () => {
    openCodebench({ code: panes[active]!.code, language: panes[active]!.language });
  });
  // <Show>-equivalent: present in the DOM only when the active pane's language is runnable,
  // never merely hidden — a prose fence (bash, json, yaml) never grows this button.
  const syncTry = () => {
    if (runnableFence(panes[active]!.language)) {
      if (!tryBtn.isConnected) actions.appendChild(tryBtn);
    } else {
      tryBtn.remove();
    }
  };
  syncTry();

  const lead = document.createElement("div");
  lead.className = "fence-group__lead";
  if (panes.length > 1) {
    panes.forEach((pane, index) => {
      const tab = document.createElement("button");
      tab.className = "fence-group__tab";
      tab.setAttribute("aria-label", `Show the ${displayLang(pane.language)} version`);
      tab.innerHTML = `${PLAY_SVG}<span>${displayLang(pane.language)}</span>`;
      tab.addEventListener("click", () => {
        active = index;
        showActive();
        for (const [i, el] of lead.querySelectorAll("button").entries())
          el.classList.toggle("fence-group__tab--active", i === active);
        syncTry();
      });
      lead.appendChild(tab);
    });
    lead.querySelector("button")?.classList.add("fence-group__tab--active");
  } else {
    const pill = document.createElement("span");
    pill.className = "fence-group__pill";
    pill.innerHTML = `${PLAY_SVG}<span>${displayLang(panes[0]!.language)}</span>`;
    lead.appendChild(pill);
  }

  bar.replaceChildren(lead, actions);
  showActive();
}

export function hydrateFenceGroups(root: ParentNode): void {
  for (const group of root.querySelectorAll("div.fence-group")) mountBar(group);
}
