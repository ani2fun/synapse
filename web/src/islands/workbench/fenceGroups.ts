/**
 * Fence-group bars, vanilla TS — one piece of state per group (the active pane), plain DOM the
 * rest of the way.
 *
 * Language tabs when adjacent plain fences differ, a lone ▶ pill otherwise; copy + "Try in
 * Editor" on the right. Try in Editor follows the SELECTED tab and only appears when the sandbox
 * speaks that language (`runnableFence`, shared with the language preference) — prose fences
 * (bash, json, yaml) still get the bar and the copy button, just not this one.
 *
 * A group opens on the reader's PINNED language when it has one (`lib/execution/pinnedLanguage`),
 * and a pin beside Copy sets it. Pinning switches every group on the page that offers the
 * language, through LANGUAGE_PINNED, so the reader never has to visit each block in turn.
 */

const PLAY_SVG =
  '<svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"></path></svg>';
const COPY_SVG =
  '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="8" y="8" width="14" height="14" rx="2" ry="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg>';
const PIN_SVG = `<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${PIN_PATHS}</svg>`;
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
import { PIN_PATHS } from "../../lib/catalog/problemFrame";
import { displayLang } from "../../lib/execution/blocks";
import { preferredIndex, runnableFence } from "../../lib/execution/language";
import { isPinned, pinLanguage, pinnedLanguage } from "../../lib/execution/pinnedLanguage";
import * as log from "../../lib/log";
import { openCodebench } from "../widgets/Codebench";
import { LANGUAGE_PINNED, type LanguagePinned } from "./contracts";

function mountBar(group: Element): void {
  const bar = group.querySelector("div.fence-group__bar");
  const panes = collectPanes(group);
  if (!bar || panes.length === 0) return;

  let active = preferredIndex(panes, pinnedLanguage());

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

  // Only a group with a choice to make carries a pin, and only on a language the sandbox runs —
  // anything else has no canonical token to store. In the DOM or not at all, like Try in Editor.
  const pin = document.createElement("button");
  pin.className = "fence-group__pin";
  pin.innerHTML = PIN_SVG;
  pin.addEventListener("click", () => pinLanguage(panes[active]!.language));
  /** The pin reads as a STATE: pressed when this tab is already what code opens in. */
  const syncPin = () => {
    const language = panes[active]!.language;
    if (panes.length < 2 || !runnableFence(language)) {
      pin.remove();
      return;
    }
    if (!pin.isConnected) actions.insertBefore(pin, copy);
    const on = isPinned(language);
    const hint = on
      ? `Code opens in ${displayLang(language)} by default`
      : `Open code in ${displayLang(language)} by default`;
    pin.classList.toggle("fence-group__pin--on", on);
    pin.setAttribute("aria-pressed", String(on));
    pin.setAttribute("aria-label", hint);
    pin.title = hint;
  };

  const lead = document.createElement("div");
  lead.className = "fence-group__lead";
  /** The one path to a tab — a click, the opening state and a pin elsewhere on the page. */
  const select = (index: number) => {
    active = index;
    panes.forEach((pane, i) => pane.figure.classList.toggle("fence-group__pane--hidden", i !== active));
    for (const [i, el] of lead.querySelectorAll("button").entries())
      el.classList.toggle("fence-group__tab--active", i === active);
    syncTry();
    syncPin();
  };

  if (panes.length > 1) {
    panes.forEach((pane, index) => {
      const tab = document.createElement("button");
      tab.className = "fence-group__tab";
      tab.setAttribute("aria-label", `Show the ${displayLang(pane.language)} version`);
      tab.innerHTML = `${PLAY_SVG}<span>${displayLang(pane.language)}</span>`;
      tab.addEventListener("click", () => select(index));
      lead.appendChild(tab);
    });
    // A group without the pinned language stays on the tab the reader left it on rather than
    // jumping back to its first — the pin is a request for one language, not a reset — and only
    // its pin repaints, since it no longer names the default.
    // A preview that re-renders drops this group; the listener goes with it.
    const onPinned = (event: Event) => {
      if (!group.isConnected) {
        window.removeEventListener(LANGUAGE_PINNED, onPinned);
        return;
      }
      const { language } = (event as CustomEvent<LanguagePinned>).detail;
      const index = preferredIndex(panes, language);
      const offers = isPinned(panes[index]!.language);
      log.debug(`fence group: pinned ${language} → ${offers ? "switch" : "stay"}`);
      select(offers ? index : active);
    };
    window.addEventListener(LANGUAGE_PINNED, onPinned);
  } else {
    const pill = document.createElement("span");
    pill.className = "fence-group__pill";
    pill.innerHTML = `${PLAY_SVG}<span>${displayLang(panes[0]!.language)}</span>`;
    lead.appendChild(pill);
  }

  bar.replaceChildren(lead, actions);
  select(active);
}

export function hydrateFenceGroups(root: ParentNode): void {
  for (const group of root.querySelectorAll("div.fence-group")) mountBar(group);
}
