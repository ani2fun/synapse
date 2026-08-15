/**
 * The ```d2 boards viewer — one source, a TREE of boards, and a reader who drives it.
 *
 * Clicking a node that carries `link:` drills into that board; ◀ ▶ ⌂ and a board menu walk back
 * out. The whole thing works identically on the card and inside the Enlarge overlay, because both
 * render the same figure and the same chrome (`Zoom.tsx` takes the chrome as a slot).
 *
 * Navigation is a DELEGATED click, not a rewrite of the SVG: d2 writes the absolute board id into
 * every anchor it emits (`href="root.layers.container"`), so a click is a map lookup against the
 * manifest. That leaves the committed SVG byte-for-byte what `d2 render` produced, and leaves the
 * anchors real links — focusable, Enter-activatable, announced as links — which is most of the
 * keyboard story for free.
 *
 * Board history is the component's own, never the browser's: a reader four levels deep still
 * leaves the lesson with one press of Back. The URL still carries the board (`?board=<slug>`,
 * written with `replaceState`) so a walkthrough is shareable — see `lib/islands/diagram/boards`.
 */
import { type ComponentChildren, h } from "preact";
import { useCallback, useEffect, useMemo, useRef, useState } from "preact/hooks";

import { DiagramEdit } from "./DiagramEdit";
import { ZoomAffordance } from "./Zoom";
import { Icon } from "../diagramlab/icons";
import { apiBase } from "../../lib/api/client";
import { fnv1a } from "../../lib/hash";
import {
  type BoardHistory,
  type BoardIndex,
  type BoardManifest,
  type BoardMeta,
  BOARD_PARAM,
  boardFromSearch,
  boardSearch,
  canGoBack,
  canGoForward,
  canNavigate,
  currentBoard,
  goBack,
  goForward,
  goHome,
  indexBoards,
  pushBoard,
  rootTitleOf,
  startHistory,
} from "../../lib/islands/diagram/boards";
import * as log from "../../lib/log";
import { watchNear } from "../workbench/lazy";

/** Where a board's SVG comes from — a drawn sidecar, or the engine in this tab. */
export interface BoardProvider {
  manifest: BoardManifest;
  svgFor(id: string): Promise<string>;
}

// ─────────────────────────────────────────────────────────────────────────────
// THE WALK
// Everything about being somewhere in a board tree and moving elsewhere in it, with no opinion
// about layout. Two surfaces render it differently — the reader's card puts the chrome under the
// figure, `/d2` floats it over a canvas — and they must not each own a copy of the navigation.
// ─────────────────────────────────────────────────────────────────────────────

export interface BoardWalk {
  index: BoardIndex;
  history: BoardHistory;
  /** The board on screen. */
  at: string;
  /** Its SVG, or null while it is being fetched or drawn. */
  svg: string | null;
  title: string;
  failed: string | null;
  menuOpen: boolean;
  setMenuOpen: (open: boolean) => void;
  show: (id: string) => void;
  back: () => void;
  forward: () => void;
  home: () => void;
  /** Delegated click for whatever element holds the figure. */
  onFigureClick: (event: MouseEvent) => void;
  /** Arrow keys / Home / Escape, to bind on whichever element owns focus. */
  onKeyDown: (event: KeyboardEvent) => void;
}

export function useBoardWalk({
  provider,
  initialSvg,
  /** The `?board=` deep link is read once, and only by the reader's copy: the editor's preview
   *  is not the page's subject and must not claim its URL. */
  ownsUrl = false,
}: {
  provider: BoardProvider;
  initialSvg?: string | null;
  ownsUrl?: boolean;
}): BoardWalk {
  const index = useMemo(() => indexBoards(provider.manifest), [provider.manifest]);
  const [history, setHistory] = useState<BoardHistory>(() => {
    const deep = ownsUrl ? boardFromSearch(window.location.search, index) : null;
    return { entries: [deep ?? index.root], at: 0 };
  });
  const [svgByBoard, setSvgByBoard] = useState<Map<string, string>>(() => {
    const seeded = new Map<string, string>();
    if (initialSvg != null) seeded.set(index.root, initialSvg);
    return seeded;
  });
  const [failed, setFailed] = useState<string | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const card = useRef<HTMLDivElement>(null);
  const mounted = useRef(false);
  const at = currentBoard(history);
  const svg = svgByBoard.get(at) ?? null;

  /**
   * A new provider means the diagram was recompiled — the `/d2` editor, on every edit.
   *
   * The drawings are stale, but the PLACE is not: an author refining the Component board should
   * still be looking at it after they type, not thrown back to the root. So the boards are
   * dropped and the trail is kept, minus any board the edit deleted.
   */
  useEffect(() => {
    if (!mounted.current) {
      mounted.current = true; // the first provider is the one the initial state was built from
      return;
    }
    setSvgByBoard(new Map());
    setFailed(null);
    setHistory((was) => {
      const entries = was.entries.filter((id) => index.get(id) != null);
      if (entries.length === 0) return startHistory(index.root);
      return { entries, at: Math.min(was.at, entries.length - 1) };
    });
  }, [provider, index]);

  // A board is fetched (or drawn) the first time it is asked for, then kept. `live` guards the
  // late arrival of a board the reader has already navigated away from.
  //
  // `drawn` is a dependency, not just a guard read inside the body: when a recompile empties the
  // cache the board on screen has not changed, so without it this effect would keep its stale
  // "already have it" answer and the figure would stay blank forever.
  const drawn = svgByBoard.has(at);
  useEffect(() => {
    if (drawn) return;
    let live = true;
    void (async () => {
      try {
        const drawn = await provider.svgFor(at);
        if (live) setSvgByBoard((was) => new Map(was).set(at, drawn));
      } catch (error) {
        if (live) setFailed(error instanceof Error ? error.message : String(error));
      }
    })();
    return () => {
      live = false;
    };
  }, [at, provider, drawn]);

  // The neighbours the reader can reach from here, warmed once this board is on screen. The
  // browser's cache is the store; this only asks for them.
  useEffect(() => {
    const board = index.get(at);
    if (board == null) return;
    const idle = window.requestIdleCallback ?? ((fn: () => void) => window.setTimeout(fn, 300));
    const handle = idle(() => {
      for (const next of board.links) {
        if (svgByBoard.has(next)) continue;
        void provider.svgFor(next).then(
          (drawn) => setSvgByBoard((was) => (was.has(next) ? was : new Map(was).set(next, drawn))),
          () => undefined, // a warm that fails costs nothing; the click will report it
        );
      }
    });
    return () => window.cancelIdleCallback?.(handle as number);
  }, [at, index, provider]);

  const show = useCallback(
    (id: string) => {
      if (index.get(id) == null) return;
      setHistory((was) => pushBoard(was, id));
      setMenuOpen(false);
    },
    [index],
  );

  // Shareable, but never a browser history entry: Back belongs to the page, not to the diagram.
  useEffect(() => {
    if (!ownsUrl) return;
    const search = boardSearch(window.location.search, index, at);
    window.history.replaceState(null, "", `${window.location.pathname}${search}${window.location.hash}`);
  }, [at, index, ownsUrl]);

  useEffect(() => {
    log.debug(`d2 boards → ${at}`);
  }, [at]);

  /** A click anywhere in the figure. d2 emits both `href` and `xlink:href`; either one is the
   *  board id when the manifest knows it, and anything else belongs to the browser. */
  const onFigureClick = useCallback(
    (event: MouseEvent) => {
      const anchor = (event.target as Element | null)?.closest?.("a");
      if (anchor == null) return;
      const href = anchor.getAttribute("xlink:href") ?? anchor.getAttribute("href") ?? "";
      if (/^[a-z][a-z0-9+.-]*:/i.test(href)) {
        // An external link the author wrote. Open it away from the lesson.
        anchor.setAttribute("target", "_blank");
        anchor.setAttribute("rel", "noopener noreferrer");
        return;
      }
      // Everything else addresses the board tree. A target the manifest does not know is a link
      // the generator already warned about; it stays inert rather than navigating to nothing.
      event.preventDefault();
      if (index.get(href) != null) show(href);
    },
    [index, show],
  );

  // Bound to whichever element owns focus — never the document. The editorial pane re-renders
  // markdown and re-hydrates over live hosts without unmounting, so a document listener would
  // survive every tab switch and keep answering against detached DOM.
  const onKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (event.key === "Escape" && menuOpen) setMenuOpen(false);
      else if (event.key === "ArrowLeft") setHistory(goBack);
      else if (event.key === "ArrowRight") setHistory(goForward);
      else if (event.key === "Home") setHistory((was) => goHome(was, index.root));
      else return;
      event.preventDefault();
    },
    [menuOpen, index],
  );

  return {
    index,
    history,
    at,
    svg,
    title: index.get(at)?.title ?? "",
    failed,
    menuOpen,
    setMenuOpen,
    show,
    back: () => setHistory(goBack),
    forward: () => setHistory(goForward),
    home: () => setHistory((was) => goHome(was, index.root)),
    onFigureClick,
    onKeyDown,
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// THE CARD — the reader's layout: figure, then chrome, inside a diagram card
// ─────────────────────────────────────────────────────────────────────────────

export function D2BoardsCard(props: {
  provider: BoardProvider;
  initialSvg?: string | null;
  ownsUrl?: boolean;
  /** The Edit pill, when this card is a lesson's figure rather than the editor's own preview. */
  edit?: ComponentChildren;
}) {
  const walk = useBoardWalk(props);
  const chrome = <BoardChrome walk={walk} />;

  return (
    <>
      {walk.failed != null && (
        <div class="diagram-error">{`D2 walkthrough failed — ${walk.failed}.`}</div>
      )}
      <div
        class="diagram diagram--boards not-prose"
        role="group"
        aria-roledescription="multi-board diagram"
        aria-label={`Diagram walkthrough, ${props.provider.manifest.boards.length} boards`}
        tabIndex={0}
        onKeyDown={walk.onKeyDown}
      >
        <ZoomAffordance
          svgHtml={walk.svg}
          chrome={chrome}
          edit={props.edit}
          onFigureClick={walk.onFigureClick}
          label={`Diagram walkthrough — ${walk.title}`}
        />
        <div
          class="diagram__figure"
          onClick={walk.onFigureClick as never}
          dangerouslySetInnerHTML={{ __html: walk.svg ?? "" }}
        ></div>
        {/* The position a stepping reader would otherwise have to infer from an injected SVG,
            whose text a screen reader would read wholesale. */}
        <p class="diagram__live" aria-live="polite">
          {walk.title}
        </p>
        {chrome}
      </div>
    </>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE CHROME — back/forward/home + breadcrumb + the board menu
//
// Two skins over one set of controls. `card` sits under the figure in a lesson and inside the
// Enlarge overlay; `bar` is the floating pill `/d2` puts over its canvas. The labels are the
// same either way, which is what keeps them the thing tests and screen readers address.
// ─────────────────────────────────────────────────────────────────────────────

/** The board list, shared by both controls — the only part of them that is genuinely identical. */
function BoardMenu({
  walk,
  listClass,
  itemClass,
}: {
  walk: BoardWalk;
  listClass: string;
  itemClass: string;
}) {
  const { index, at, show, setMenuOpen } = walk;
  return (
    <ul class={listClass} role="listbox">
      {index.order.map((board) => (
        <li key={board.id}>
          <button
            class={board.id === at ? `${itemClass} ${itemClass}--at` : itemClass}
            role="option"
            aria-selected={board.id === at}
            // Depth reads as indentation, so a tree stays a tree in a flat list.
            style={board.parent == null ? undefined : "padding-left: 22px"}
            onClick={() => {
              show(board.id);
              setMenuOpen(false);
            }}
          >
            {board.title}
          </button>
        </li>
      ))}
    </ul>
  );
}

/** The reader's control: it sits under the figure in a lesson, in the transport's own idiom. */
export function BoardChrome({ walk }: { walk: BoardWalk }) {
  const { index, history, at, menuOpen, setMenuOpen, show, back, forward, home } = walk;
  const trail = index.trail(at);

  if (!canNavigate(index)) return null;

  return (
    <div class="boards-bar">
      <div class="transport boards-bar__nav">
        <button class="transport__btn" aria-label="Back" title="Back" disabled={!canGoBack(history)} onClick={back}>
          ‹
        </button>
        <button
          class="transport__btn"
          aria-label="Forward"
          title="Forward"
          disabled={!canGoForward(history)}
          onClick={forward}
        >
          ›
        </button>
        <button
          class="transport__btn"
          aria-label="Root board"
          title="Root board"
          disabled={at === index.root}
          onClick={home}
        >
          ⌂
        </button>
      </div>
      <nav class="boards-bar__trail" aria-label="Diagram breadcrumb">
        {trail.map((board: BoardMeta, i: number) => (
          <span key={board.id}>
            {i > 0 && (
              <span class="boards-bar__sep" aria-hidden="true">
                ›
              </span>
            )}
            {board.id === at ? (
              <span class="boards-bar__here" aria-current="true">
                {board.title}
              </span>
            ) : (
              <button class="boards-bar__crumb" onClick={() => show(board.id)}>
                {board.title}
              </button>
            )}
          </span>
        ))}
      </nav>
      <div class="boards-bar__menu">
        <button
          class="transport__btn"
          aria-haspopup="listbox"
          aria-expanded={menuOpen}
          aria-label="Jump to a board"
          title="Jump to a board"
          onClick={() => setMenuOpen(!menuOpen)}
        >
          ☰
        </button>
        {menuOpen && (
          <BoardMenu walk={walk} listClass="boards-menu" itemClass="boards-menu__item" />
        )}
      </div>
    </div>
  );
}

/**
 * The floating control: `/d2`'s canvas, and its Enlarge overlay.
 *
 * The board you are ON is the menu button — its name and the list icon are one control, so the
 * bar answers "where am I" and "where else can I go" in the same place instead of putting the
 * answer at one end and the way to change it at the other. The ancestors stay crumbs, because
 * stepping back up is a different move from jumping sideways.
 */
export function BoardBar({ walk }: { walk: BoardWalk }) {
  const { index, history, at, menuOpen, setMenuOpen, show, back, forward, home } = walk;
  const trail = index.trail(at);
  const ancestors = trail.slice(0, -1);
  const here = trail[trail.length - 1];

  if (!canNavigate(index)) return null;


  return (
    <div class="bbar">
      <div class="bbar__nav">
        <button class="bnav" aria-label="Back" title="Back" disabled={!canGoBack(history)} onClick={back}>
          <Icon name="left" size={15} />
        </button>
        <button
          class="bnav"
          aria-label="Forward"
          title="Forward"
          disabled={!canGoForward(history)}
          onClick={forward}
        >
          <Icon name="right" size={15} />
        </button>
        <button
          class="bnav"
          aria-label="Root board"
          title="Root board"
          disabled={at === index.root}
          onClick={home}
        >
          <Icon name="home" size={14} />
        </button>
      </div>
      <span class="bbar__rule"></span>
      {ancestors.length > 0 && (
        <nav class="bbar__trail" aria-label="Diagram breadcrumb">
          {ancestors.map((board: BoardMeta) => (
            <span key={board.id}>
              <button class="crumb" onClick={() => show(board.id)}>
                {board.title}
              </button>
              <span class="bbar__sep" aria-hidden="true">
                /
              </span>
            </span>
          ))}
        </nav>
      )}
      <div class="bbar__menu">
        <button
          class={menuOpen ? "bhere is-on" : "bhere"}
          aria-haspopup="listbox"
          aria-expanded={menuOpen}
          aria-label="Jump to a board"
          title="Jump to a board"
          onClick={() => setMenuOpen(!menuOpen)}
        >
          <span class="bhere__t" aria-current="true">
            {here?.title ?? ""}
          </span>
          <Icon name="list" size={14} />
        </button>
        {menuOpen && <BoardMenu walk={walk} listClass="bmenu" itemClass="bmenu__i" />}
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// PROVIDERS — the two ways a board's SVG arrives
// ─────────────────────────────────────────────────────────────────────────────

/** Drawn boards, served from the lesson's own `_d2/<fence>/` sidecar. */
export function sidecarProvider(
  manifest: BoardManifest,
  fence: string,
  lessonPath: string,
): BoardProvider {
  const cache = new Map<string, Promise<string>>();
  return {
    manifest,
    svgFor(id) {
      const board = manifest.boards.find((entry) => entry.id === id);
      if (board == null) return Promise.reject(new Error(`no board ${id}`));
      const held = cache.get(id);
      if (held != null) return held;
      const url =
        `${apiBase()}/api/synapse/d2/${encodeURIComponent(fence)}/${encodeURIComponent(`${board.slug}.svg`)}` +
        `?lesson=${encodeURIComponent(lessonPath)}`;
      const pending = fetch(url).then((response) => {
        if (!response.ok) throw new Error(`board ${board.slug} is not drawn`);
        return response.text();
      });
      cache.set(id, pending);
      return pending;
    },
  };
}

/**
 * Boards compiled in this tab — the fallback for a fence its repo has not drawn yet, and the
 * ordinary path for the `/d2` editor.
 *
 * The compile happens once; each board renders the first time it is shown. Both go through the
 * renderer's queue, because one worker serves the whole page.
 */
export async function engineProvider(source: string, meta: string): Promise<BoardProvider> {
  const { compileD2Boards, renderD2Board } = await import("../../lib/islands/diagram/d2");
  const walked = await compileD2Boards(source, rootTitleOf(meta));
  const hash = fnv1a(source);
  const cache = new Map<string, Promise<string>>();
  const manifest: BoardManifest = {
    generator: 0, // never written to disk — this one only ever lives in the tab that built it
    source: hash,
    root: walked[0]?.id ?? "root",
    boards: walked.map(({ id, slug, title, parent, links }) => ({ id, slug, title, parent, links })),
    warnings: [],
  };
  return {
    manifest,
    svgFor(id) {
      const board = walked.find((entry) => entry.id === id);
      if (board == null) return Promise.reject(new Error(`no board ${id}`));
      const held = cache.get(id);
      if (held != null) return held;
      const pending = renderD2Board(board, hash);
      cache.set(id, pending);
      return pending;
    },
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// THE HOST — what `hydrateDiagrams` mounts over a `.d2-boards` placeholder
// ─────────────────────────────────────────────────────────────────────────────

export interface BoardsHostProps {
  /** The drawn shape: a manifest, the fence's directory, and the lesson to fetch the rest from. */
  drawn: { manifest: BoardManifest; fence: string; lessonPath: string; rootSvg: string } | null;
  /** The undrawn shape: the source to compile in this tab. */
  raw: { source: string; meta: string } | null;
  /** The element the placeholder occupied, watched so nothing compiles off-screen. */
  host: HTMLElement;
  /** Which d2 fence this is in the lesson — what the Edit pill points at. */
  fenceAt?: number;
  fenceCount?: number;
}

/**
 * Resolves which provider this walkthrough gets, then hands over to the card.
 *
 * A drawn walkthrough is ready at first paint and never touches the engine. An undrawn one waits
 * until the reader is near it before compiling — one worker serves every diagram on the page, so
 * a figure at the top must not queue behind the last one in the document.
 */
export function D2BoardsHost({ drawn, raw, host, fenceAt, fenceCount }: BoardsHostProps) {
  const [provider, setProvider] = useState<BoardProvider | null>(() =>
    drawn == null ? null : sidecarProvider(drawn.manifest, drawn.fence, drawn.lessonPath),
  );
  const [failed, setFailed] = useState<string | null>(null);
  const [near, setNear] = useState(false);
  const started = useRef(false);

  useEffect(() => {
    if (provider != null || raw == null) return;
    const watch = watchNear(host, (isNear) => {
      if (isNear) setNear(true);
    });
    return () => watch?.disconnect();
  }, []);

  useEffect(() => {
    if (!near || started.current || provider != null || raw == null) return;
    started.current = true;
    void engineProvider(raw.source, raw.meta).then(setProvider, (error: unknown) =>
      setFailed(error instanceof Error ? error.message : String(error)),
    );
  }, [near]);

  if (failed != null) {
    // Loud, with the source to fix — never a blank figure (ADR-S026).
    return (
      <div class="diagram-error">
        {`D2 walkthrough failed — ${failed}.`}
        <details>
          <summary>diagram source</summary>
          <pre>{raw?.source ?? ""}</pre>
        </details>
      </div>
    );
  }
  if (provider == null) return <div class="diagram diagram--boards not-prose" />;
  return (
    <D2BoardsCard
      provider={provider}
      initialSvg={drawn?.rootSvg ?? null}
      ownsUrl
      edit={
        fenceAt == null ? undefined : <DiagramEdit lang="d2" at={fenceAt} count={fenceCount ?? 1} />
      }
    />
  );
}

export { BOARD_PARAM };
