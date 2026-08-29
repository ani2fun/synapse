// The board graph and the walk through it. The viewer is a thin shell over this, so everything
// that can actually be wrong — where a click lands, what the breadcrumb says, what a shared URL
// reopens, what a corrupt manifest does — is decided here and asserted here.
import { describe, expect, it } from "vitest";

import {
  type BoardManifest,
  type CompiledBoard,
  GENERATOR_VERSION,
  boardFromSearch,
  boardSearch,
  boardSlug,
  boardsOf,
  canGoBack,
  canGoForward,
  canStepBack,
  canStepForward,
  canNavigate,
  currentBoard,
  decodeManifest,
  fenceName,
  goBack,
  goForward,
  goHome,
  indexBoards,
  stepBoard,
  walkBack,
  walkForward,
  isBoardsFence,
  pushBoard,
  resolveBoardLink,
  rootTitleOf,
  saltForBoard,
  startHistory,
} from "./boards";

// A C4 stack: Context → Container → Component, the shape the whole feature exists for.
const MANIFEST: BoardManifest = {
  generator: GENERATOR_VERSION,
  source: "76e32334",
  root: "root",
  boards: [
    { id: "root", slug: "root", title: "Context", parent: null, links: ["root.layers.container"] },
    {
      id: "root.layers.container",
      slug: "container",
      title: "Container",
      parent: "root",
      links: ["root.layers.component"],
    },
    {
      id: "root.layers.component",
      slug: "component",
      title: "Component",
      parent: "root.layers.container",
      links: [],
    },
  ],
  warnings: [],
};

const index = indexBoards(MANIFEST);

describe("the fence vocabulary", () => {
  it("opts in on the bare marker only", () => {
    expect(isBoardsFence("boards")).toBe(true);
    expect(isBoardsFence('boards name="x"')).toBe(true);
    expect(isBoardsFence("")).toBe(false);
    expect(isBoardsFence(null)).toBe(false);
    // A substring is not the marker — `keyboards` must not turn a diagram into a walkthrough.
    expect(isBoardsFence("keyboards")).toBe(false);
    expect(isBoardsFence("boardsy")).toBe(false);
    // Case sensitive, like `run` and `solution`.
    expect(isBoardsFence("BOARDS")).toBe(false);
  });

  it("reads quoted and bare options", () => {
    expect(fenceName('boards name="url-shortener"')).toBe("url-shortener");
    expect(fenceName("boards name=url-shortener")).toBe("url-shortener");
    expect(fenceName("boards")).toBeNull();
    expect(rootTitleOf('boards root="System Context"')).toBe("System Context");
    expect(rootTitleOf("boards")).toBeNull();
  });
});

describe("board slugs", () => {
  it("drop the kind segments a reader never sees", () => {
    expect(boardSlug("root")).toBe("root");
    expect(boardSlug("root.layers.container")).toBe("container");
    expect(boardSlug("root.layers.a.layers.b")).toBe("a-b");
    expect(boardSlug("root.steps.one")).toBe("one");
  });

  it("stay a single safe path segment", () => {
    // A slug is joined to a lesson directory server-side, so nothing may escape it.
    expect(boardSlug("root.layers...")).not.toContain(".");
    expect(boardSlug("root.layers.../..")).not.toContain("/");
    expect(boardSlug("root.layers.Café Noir")).toBe("caf-noir");
    expect(boardSlug("root.layers.!!!")).toBe("board");
  });

  it("salt every board of one diagram differently", () => {
    const ids = ["root", "root.layers.container", "root.layers.component"];
    const salts = ids.map((id) => saltForBoard("abcd1234", id));
    expect(new Set(salts).size).toBe(ids.length);
  });
});

describe("the board walk", () => {
  const leaf = (name: string, link?: string): CompiledBoard => ({
    name,
    shapes: link ? [{ link }] : [],
  });

  it("skips a folder-only board but keeps walking through it", () => {
    const diagram: CompiledBoard = {
      name: "",
      layers: [
        {
          name: "group",
          isFolderOnly: true,
          layers: [leaf("inner")],
        },
      ],
    };
    const boards = boardsOf(diagram, "Top");
    expect(boards.map((b) => b.id)).toEqual(["root", "root.layers.group.layers.inner"]);
    // The child points past the board that never rendered, so the breadcrumb stays walkable.
    expect(boards[1]!.parent).toBe("root");
  });

  it("titles a board from its key and the root from the fence", () => {
    const boards = boardsOf({ name: "", layers: [leaf("redirect_handler")] }, "System Context");
    expect(boards.map((b) => b.title)).toEqual(["System Context", "Redirect Handler"]);
    expect(boardsOf({ name: "" })[0]!.title).toBe("Overview");
  });

  it("keeps internal links and drops external ones", () => {
    const diagram: CompiledBoard = {
      name: "",
      shapes: [{ link: "root.layers.x" }, { link: "https://d2lang.com" }],
      layers: [leaf("x")],
    };
    expect(boardsOf(diagram).find((b) => b.id === "root")!.links).toEqual(["root.layers.x"]);
  });

  it("disambiguates colliding slugs so the set stays injective", () => {
    // `a-b` as one key, and `a` nesting `b`, both reduce to the same stem.
    const diagram: CompiledBoard = {
      name: "",
      layers: [leaf("a-b"), { name: "a", layers: [leaf("b")] }],
    };
    const slugs = boardsOf(diagram).map((b) => b.slug);
    expect(new Set(slugs).size).toBe(slugs.length);
    expect(slugs).toContain("a-b");
    expect(slugs).toContain("a-b-2");
  });
});

describe("the manifest", () => {
  it("round-trips a good one", () => {
    expect(decodeManifest(JSON.parse(JSON.stringify(MANIFEST)))).toEqual(MANIFEST);
  });

  it("treats anything it cannot read as not-drawn-yet", () => {
    // Each of these degrades to the client renderer rather than rendering something wrong.
    expect(decodeManifest(null)).toBeNull();
    expect(decodeManifest("<!doctype html>")).toBeNull();
    expect(decodeManifest({ ...MANIFEST, generator: GENERATOR_VERSION + 1 })).toBeNull();
    expect(decodeManifest({ ...MANIFEST, generator: GENERATOR_VERSION - 1 })).toBeNull();
    expect(decodeManifest({ ...MANIFEST, boards: [] })).toBeNull();
    expect(decodeManifest({ ...MANIFEST, boards: [{ id: "root" }] })).toBeNull();
    // A manifest whose root board is missing would open on nothing.
    expect(decodeManifest({ ...MANIFEST, root: "root.layers.nope" })).toBeNull();
  });

  it("tolerates a missing warnings list", () => {
    const { warnings: _dropped, ...without } = MANIFEST;
    expect(decodeManifest(without)?.warnings).toEqual([]);
  });
});

describe("whether there is anywhere to go", () => {
  it("says yes for a tree of boards", () => {
    expect(canNavigate(index)).toBe(true);
  });

  it("says no for a lone board", () => {
    // A plain ```d2 figure, and equally a `boards` fence someone wrote with no layers. Both
    // skins render nothing when this is false, so the figure carries no dead controls.
    const alone = indexBoards({ ...MANIFEST, boards: [MANIFEST.boards[0]!], root: MANIFEST.root });
    expect(canNavigate(alone)).toBe(false);
  });
});

describe("the breadcrumb", () => {
  it("reads root first, down to the board on screen", () => {
    expect(index.trail("root.layers.component").map((b) => b.title)).toEqual([
      "Context",
      "Container",
      "Component",
    ]);
  });

  it("is empty for a board nobody knows", () => {
    expect(index.trail("root.layers.nope")).toEqual([]);
  });
});

describe("the walk through the boards", () => {
  it("moves forward and back like a browser", () => {
    let history = startHistory("root");
    expect(canGoBack(history)).toBe(false);
    expect(canGoForward(history)).toBe(false);

    history = pushBoard(history, "root.layers.container");
    history = pushBoard(history, "root.layers.component");
    expect(currentBoard(history)).toBe("root.layers.component");
    expect(canGoBack(history)).toBe(true);

    history = goBack(history);
    expect(currentBoard(history)).toBe("root.layers.container");
    expect(canGoForward(history)).toBe(true);

    history = goForward(history);
    expect(currentBoard(history)).toBe("root.layers.component");
    expect(canGoForward(history)).toBe(false);
  });

  it("truncates the forward tail on a new push", () => {
    let history = startHistory("root");
    history = pushBoard(history, "root.layers.container");
    history = pushBoard(history, "root.layers.component");
    history = goBack(history);
    history = goBack(history);
    history = pushBoard(history, "root.layers.container");
    expect(history.entries).toEqual(["root", "root.layers.container"]);
    expect(canGoForward(history)).toBe(false);
  });

  it("ignores a click on the board already showing", () => {
    const history = pushBoard(startHistory("root"), "root");
    expect(history.entries).toEqual(["root"]);
  });

  it("keeps the trail when going home, so Back still returns", () => {
    let history = pushBoard(startHistory("root"), "root.layers.component");
    history = goHome(history, "root");
    expect(currentBoard(history)).toBe("root");
    expect(canGoBack(history)).toBe(true);
    expect(currentBoard(goBack(history))).toBe("root.layers.component");
  });

  it("refuses to walk off either end", () => {
    const history = startHistory("root");
    expect(goBack(history)).toBe(history);
    expect(goForward(history)).toBe(history);
  });
});

describe("the URL", () => {
  it("round-trips a board through the query string", () => {
    const search = boardSearch("", index, "root.layers.container");
    expect(search).toBe("?board=container");
    expect(boardFromSearch(search, index)).toBe("root.layers.container");
  });

  it("drops the parameter for the root board", () => {
    expect(boardSearch("?board=container", index, "root")).toBe("");
    expect(boardFromSearch("", index)).toBeNull();
  });

  it("leaves every other parameter alone", () => {
    expect(boardSearch("?tab=editorial", index, "root.layers.container")).toBe(
      "?tab=editorial&board=container",
    );
    expect(boardSearch("?tab=editorial&board=container", index, "root")).toBe("?tab=editorial");
  });

  it("ignores a slug nobody has, so a stale link still opens the diagram", () => {
    expect(boardFromSearch("?board=renamed", index)).toBeNull();
  });
});

describe("resolving a link the author wrote", () => {
  it("takes an absolute path as given", () => {
    expect(resolveBoardLink("root.layers.c", "root.layers.b")).toBe("root.layers.c");
    expect(resolveBoardLink("root", "root.layers.b")).toBe("root");
  });

  it("reads a bare path as relative to the board it sits in", () => {
    // The trap this whole audit exists for: at the root this is right, one level down it is not.
    expect(resolveBoardLink("layers.c", "root")).toBe("root.layers.c");
    expect(resolveBoardLink("layers.c", "root.layers.b")).toBe("root.layers.b.layers.c");
  });

  it("steps up one board per leading underscore", () => {
    expect(resolveBoardLink("_.layers.c", "root.layers.b")).toBe("root.layers.c");
    expect(resolveBoardLink("_.layers.d", "root.layers.a.layers.b")).toBe("root.layers.a.layers.d");
    expect(resolveBoardLink("_._.layers.d", "root.layers.a.layers.b")).toBe("root.layers.d");
    expect(resolveBoardLink("_", "root.layers.b")).toBe("root");
  });

  it("leaves anything with a scheme to the browser", () => {
    expect(resolveBoardLink("https://d2lang.com", "root")).toBeNull();
    expect(resolveBoardLink("mailto:a@b.c", "root")).toBeNull();
    expect(resolveBoardLink("", "root")).toBeNull();
  });
});

// ── STEPPING: the transport on a page that has been nowhere ──────
// History alone leaves both arrows disabled at first paint, which reads as broken controls. These
// pin the fallback: with no history to spend, the arrows walk the manifest's order.

describe("stepping the boards", () => {
  it("stepsForwardThroughWalkOrderAndStopsAtTheEnd", () => {
    expect(stepBoard(index, "root", 1)).toBe("root.layers.container");
    expect(stepBoard(index, "root.layers.container", 1)).toBe("root.layers.component");
    expect(stepBoard(index, "root.layers.component", 1)).toBeNull();
  });

  it("stepsBackwardAndStopsAtTheRoot", () => {
    expect(stepBoard(index, "root.layers.component", -1)).toBe("root.layers.container");
    expect(stepBoard(index, "root", -1)).toBeNull();
  });

  it("anUnknownBoardStepsNowhere", () => {
    expect(stepBoard(index, "root.layers.nope", 1)).toBeNull();
  });

  it("aFreshlyLoadedRootCanStepForwardButNotBack", () => {
    const fresh = startHistory("root");
    expect(canGoForward(fresh)).toBe(false); // no history…
    expect(canStepForward(index, fresh)).toBe(true); // …but a board to step to
    expect(canStepBack(index, fresh)).toBe(false); // nothing before the root
  });

  it("aDeepLinkedBoardCanStepBackOutWithNoHistory", () => {
    // `?board=component` lands mid-tree with an empty trail; the reader must not be stranded.
    const deep = startHistory("root.layers.component");
    expect(canGoBack(deep)).toBe(false);
    expect(canStepBack(index, deep)).toBe(true);
    expect(currentBoard(walkBack(index, deep))).toBe("root.layers.container");
  });

  it("historyWinsOverSteppingWhereverItExists", () => {
    // Jump to the deepest board, then back: Back must return where the reader CAME from, not to
    // whatever happens to sit beside it in the manifest.
    let history = startHistory("root");
    history = pushBoard(history, "root.layers.component");
    expect(currentBoard(walkBack(index, history))).toBe("root");
    history = walkBack(index, history);
    expect(currentBoard(walkForward(index, history))).toBe("root.layers.component");
  });

  it("walkingPastEitherEndIsANoOp", () => {
    const last = startHistory("root.layers.component");
    expect(walkForward(index, last)).toBe(last);
    const first = startHistory("root");
    expect(walkBack(index, first)).toBe(first);
  });
});
