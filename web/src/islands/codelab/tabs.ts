/**
 * The code lab's open buffers, as a pure model: a list of tabs and which one is on screen.
 *
 * Every operation returns a NEW `Tabs` and never mutates its input, so the page can hold it in
 * one `useState` and the tests can assert on values. Two rules the operations keep between them:
 * there is always at least one tab (closing the last opens a fresh one, as an editor with no
 * buffer would have nothing for Run to run), and `active` always names a tab that exists.
 */
import { DEFAULT_LANGUAGE, starterFor } from "./starters";

export interface Tab {
  readonly id: string;
  readonly name: string;
  readonly language: string;
  readonly source: string;
}

export interface Tabs {
  readonly list: readonly Tab[];
  readonly active: string;
}

/** `Code N` for the smallest N no open tab already uses, so closing "Code 2" frees the name. */
export function nextName(list: readonly Tab[]): string {
  const taken = new Set(list.map((tab) => tab.name));
  let n = 1;
  while (taken.has(`Code ${n}`)) n += 1;
  return `Code ${n}`;
}

/** An id no open tab uses. Counter-based rather than random, so tests are deterministic. */
function nextId(list: readonly Tab[]): string {
  const taken = new Set(list.map((tab) => tab.id));
  let n = list.length + 1;
  while (taken.has(`t${n}`)) n += 1;
  return `t${n}`;
}

export function freshTab(list: readonly Tab[], language = DEFAULT_LANGUAGE, source?: string, name?: string): Tab {
  return { id: nextId(list), name: name ?? nextName(list), language, source: source ?? starterFor(language) };
}

export function initialTabs(): Tabs {
  const tab = freshTab([]);
  return { list: [tab], active: tab.id };
}

export function activeTab(tabs: Tabs): Tab {
  return tabs.list.find((tab) => tab.id === tabs.active) ?? tabs.list[0]!;
}

/** Open a tab after the others and put it on screen. */
export function addTab(tabs: Tabs, language: string, source?: string, name?: string): Tabs {
  const tab = freshTab(tabs.list, language, source, name);
  return { list: [...tabs.list, tab], active: tab.id };
}

/** Close a tab. Closing the one on screen moves to its right-hand neighbour, else its left. */
export function closeTab(tabs: Tabs, id: string): Tabs {
  const at = tabs.list.findIndex((tab) => tab.id === id);
  if (at < 0) return tabs;
  const list = tabs.list.filter((tab) => tab.id !== id);
  if (list.length === 0) return initialTabs();
  if (tabs.active !== id) return { list, active: tabs.active };
  const next = list[Math.min(at, list.length - 1)]!;
  return { list, active: next.id };
}

export function selectTab(tabs: Tabs, id: string): Tabs {
  return tabs.list.some((tab) => tab.id === id) ? { ...tabs, active: id } : tabs;
}

function update(tabs: Tabs, id: string, change: (tab: Tab) => Tab): Tabs {
  return { ...tabs, list: tabs.list.map((tab) => (tab.id === id ? change(tab) : tab)) };
}

export function setSource(tabs: Tabs, id: string, source: string): Tabs {
  return update(tabs, id, (tab) => ({ ...tab, source }));
}

/**
 * Switch a tab's language. A buffer still holding the OLD language's starter takes the new one's,
 * because Hello World in Java is no use to someone who just picked Go; anything the reader wrote
 * is kept as it is.
 */
export function setLanguage(tabs: Tabs, id: string, language: string): Tabs {
  return update(tabs, id, (tab) => {
    const untouched = tab.source.trim() === "" || tab.source === starterFor(tab.language);
    return { ...tab, language, source: untouched ? starterFor(language) : tab.source };
  });
}

/**
 * Read a stored or shared list back. Anything malformed is dropped rather than trusted, and an
 * empty result is null, so the caller falls back to a fresh tab instead of an empty strip.
 */
export function parseTabs(raw: unknown, active: unknown, known: (language: string) => boolean): Tabs | null {
  if (!Array.isArray(raw)) return null;
  const list: Tab[] = [];
  for (const item of raw) {
    if (item == null || typeof item !== "object") continue;
    const { id, name, language, source } = item as Record<string, unknown>;
    if (typeof name !== "string" || typeof language !== "string" || typeof source !== "string") continue;
    if (!known(language)) continue;
    const safeId = typeof id === "string" && !list.some((tab) => tab.id === id) ? id : nextId(list);
    list.push({ id: safeId, name: name.slice(0, 60) || nextName(list), language, source });
  }
  if (list.length === 0) return null;
  const on = typeof active === "string" && list.some((tab) => tab.id === active) ? active : list[0]!.id;
  return { list, active: on };
}

/**
 * Open a shared link's tabs beside the reader's own. Nothing they wrote is replaced: only tabs
 * still holding an untouched starter make way, so opening a link never costs work. The shared tab
 * that was on screen is put on screen.
 */
export function adoptShared(
  tabs: Tabs,
  shared: readonly { name: string; language: string; source: string }[],
  active: number,
): Tabs {
  let list: Tab[] = tabs.list.filter((tab) => tab.source !== starterFor(tab.language));
  let on = "";
  shared.forEach((incoming, index) => {
    const tab = freshTab(list, incoming.language, incoming.source, incoming.name.slice(0, 60) || undefined);
    list = [...list, tab];
    if (index === active) on = tab.id;
  });
  if (list.length === 0) return initialTabs();
  return { list, active: on || list[list.length - 1]!.id };
}
