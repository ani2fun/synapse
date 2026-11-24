import { describe, expect, it } from "vitest";

import { starterFor } from "./starters";
import {
  activeTab,
  addTab,
  adoptShared,
  closeTab,
  initialTabs,
  nextName,
  parseTabs,
  selectTab,
  setLanguage,
  setSource,
} from "./tabs";

const known = (language: string) => ["python", "go", "java"].includes(language);

describe("code lab tabs", () => {
  it("opens on one Python tab holding the starter", () => {
    const tabs = initialTabs();
    expect(tabs.list).toHaveLength(1);
    expect(activeTab(tabs)).toMatchObject({ name: "Code 1", language: "python", source: starterFor("python") });
  });

  it("names a new tab with the smallest free number and puts it on screen", () => {
    let tabs = addTab(initialTabs(), "go");
    expect(activeTab(tabs).name).toBe("Code 2");
    tabs = addTab(tabs, "go");
    tabs = closeTab(tabs, tabs.list[1]!.id);
    expect(nextName(tabs.list)).toBe("Code 2");
  });

  it("closing the tab on screen moves right, then left", () => {
    let tabs = addTab(addTab(initialTabs(), "go"), "java");
    const [first, second, third] = tabs.list;
    tabs = selectTab(tabs, second!.id);
    tabs = closeTab(tabs, second!.id);
    expect(tabs.active).toBe(third!.id);
    tabs = closeTab(tabs, third!.id);
    expect(tabs.active).toBe(first!.id);
  });

  it("closing a tab that is not on screen keeps the one that is", () => {
    let tabs = addTab(initialTabs(), "go");
    const on = tabs.active;
    tabs = closeTab(tabs, tabs.list[0]!.id);
    expect(tabs.active).toBe(on);
  });

  it("closing the last tab leaves a fresh one, never an empty strip", () => {
    const tabs = initialTabs();
    const after = closeTab(tabs, tabs.active);
    expect(after.list).toHaveLength(1);
    expect(activeTab(after).source).toBe(starterFor("python"));
  });

  it("a language switch replaces an untouched starter and keeps the reader's own code", () => {
    let tabs = initialTabs();
    tabs = setLanguage(tabs, tabs.active, "go");
    expect(activeTab(tabs).source).toBe(starterFor("go"));
    tabs = setSource(tabs, tabs.active, "package main // mine");
    tabs = setLanguage(tabs, tabs.active, "java");
    expect(activeTab(tabs)).toMatchObject({ language: "java", source: "package main // mine" });
  });

  it("reads a stored list back, dropping what it cannot trust", () => {
    const parsed = parseTabs(
      [
        { id: "a", name: "Mine", language: "go", source: "x" },
        { id: "a", name: "Dup id", language: "python", source: "y" },
        { id: "c", name: "Unknown", language: "cobol", source: "z" },
        "garbage",
      ],
      "missing",
      known,
    );
    expect(parsed?.list.map((tab) => [tab.name, tab.language])).toEqual([
      ["Mine", "go"],
      ["Dup id", "python"],
    ]);
    expect(new Set(parsed?.list.map((tab) => tab.id)).size).toBe(2);
    expect(parsed?.active).toBe("a");
    expect(parseTabs([], null, known)).toBeNull();
    expect(parseTabs("nope", null, known)).toBeNull();
  });

  it("a shared link opens beside the reader's own work and replaces only untouched starters", () => {
    let tabs = addTab(initialTabs(), "go");
    tabs = setSource(tabs, tabs.active, "package main // mine");
    const after = adoptShared(tabs, [
      { name: "Shared A", language: "python", source: "print(1)" },
      { name: "Shared B", language: "java", source: "class Main {}" },
    ], 1);
    expect(after.list.map((tab) => tab.name)).toEqual(["Code 2", "Shared A", "Shared B"]);
    expect(new Set(after.list.map((tab) => tab.id)).size).toBe(3);
    expect(activeTab(after).name).toBe("Shared B");
  });
});
