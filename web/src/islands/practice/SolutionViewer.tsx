/**
 * The read-only solution viewer. A `.solution-block` placeholder — one or more `solution` fences
 * grouped by render.ts — becomes a read-only Monaco behind the SAME language dropdown as the
 * workbench's language pill; switching swaps the buffer + tokenizer in place. "Copy to editor"
 * dispatches LOAD_CODE ON the workbench root (the `workbench/contracts.ts` contract), so the
 * ACTIVE tab's code lands in its matching language tab on the right — language-exact across the
 * pane boundary.
 *
 * Two callers: the embedded practice widget mounts it REVEALED; the problem-page editorial
 * mounts it behind a reveal gate. The component itself is identical either way — the gate is the
 * editorial's, not the viewer's.
 */
import { useEffect, useRef, useState } from "preact/hooks";

import { displayLang } from "../../lib/execution/blocks";
import type { Variant } from "../../lib/execution/blocks";
import { canonicalLang, preferredIndex } from "../../lib/execution/language";
import type { EditorHandle } from "../../lib/islands/editor/monaco";
import { WB_LANGUAGE_KEY, get as storageGet, set as storageSet } from "../../lib/storage";
import { LOAD_CODE, RELAYOUT } from "../workbench/contracts";
import type { LoadCode } from "../workbench/contracts";
import * as log from "../../lib/log";

/** The editor height rule: clamp to a line count, with a floor and a ceiling. */
function defaultHeightPx(source: string): number {
  const lines = source.split("\n").length;
  return Math.min(Math.max(lines * 20 + 28, 64), 520);
}

const CHEVRON = (
  <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
    <path d="m6 9 6 6 6-6"></path>
  </svg>
);

export interface SolutionViewerProps {
  variants: Variant[];
  /** The workbench root LOAD_CODE targets — the practice widget's own root, or the problem page's
   *  right-pane root. `null` (no workbench) makes Copy-to-editor a harmless no-op. */
  workbenchRoot: () => HTMLElement | null;
}

export function SolutionViewer({ variants, workbenchRoot }: SolutionViewerProps) {
  const start = preferredIndex(variants, storageGet(WB_LANGUAGE_KEY));
  const [active, setActive] = useState(start);
  const [menuOpen, setMenuOpen] = useState(false);
  const host = useRef<HTMLDivElement>(null);
  const mounted = useRef<EditorHandle | null>(null);
  const activeRef = useRef(active);
  activeRef.current = active;

  const variant = variants[active]!;

  // Lazy Monaco, mounted VISIBLE (the reveal already happened for the gated case, so it measures
  // right away). Dynamic import keeps monaco a lazy chunk.
  useEffect(() => {
    const node = host.current;
    if (!node || mounted.current) return;
    const first = variants[start]!;
    const dark = document.documentElement.classList.contains("dark");
    void (async () => {
      const { createEditor } = await import("../../lib/islands/editor/monaco");
      if (mounted.current) return;
      mounted.current = createEditor(node, {
        value: first.source,
        language: first.language,
        readOnly: true,
        dark,
      });
      log.debug(`solution viewer monaco mounted (${first.language})`);
    })();
    return () => {
      mounted.current?.dispose();
      mounted.current = null;
    };
  }, []);

  // A viewer inside a collapsed editorial section (or a hidden tab) mounts 0×0 and renders no
  // lines; the reveal broadcasts RELAYOUT and re-measuring here makes the code appear.
  useEffect(() => {
    const onRelayout = () => mounted.current?.relayout();
    window.addEventListener(RELAYOUT, onRelayout);
    return () => window.removeEventListener(RELAYOUT, onRelayout);
  }, []);

  // Theme follows the toggle: the theme island flips `.dark` on `<html>`; observe it.
  useEffect(() => {
    const observer = new MutationObserver(() =>
      mounted.current?.setTheme(document.documentElement.classList.contains("dark")),
    );
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, []);

  const switchTo = (index: number) => {
    if (index === activeRef.current) return;
    setActive(index);
    const v = variants[index]!;
    mounted.current?.setValue(v.source);
    mounted.current?.setLanguage(v.language);
  };

  const copyToEditor = () => {
    const root = workbenchRoot();
    if (!root) {
      log.debug("copy-to-editor: no workbench root in reach");
      return;
    }
    const v = variants[activeRef.current]!;
    log.debug(`solution copied toward the ${v.language} tab`);
    const detail: LoadCode = { language: v.language, code: v.source };
    root.dispatchEvent(new CustomEvent(LOAD_CODE, { detail }));
  };

  const height = Math.max(...variants.map((v) => defaultHeightPx(v.source)));

  const langChrome =
    variants.length > 1 ? (
      <div class="wb__lang">
        <button
          class="wb__lang-pill wb__lang-pill--btn"
          aria-label="Solution language"
          onClick={() => setMenuOpen((o) => !o)}
        >
          <span>{displayLang(variant.language)}</span>
          {CHEVRON}
        </button>
        {menuOpen && (
          <div>
            <div class="wb__lang-scrim" onClick={() => setMenuOpen(false)}></div>
            <div class="wb__lang-menu">
              {variants.map((v, i) => (
                <button
                  class={`wb__lang-opt${active === i ? " wb__lang-opt--active" : ""}`}
                  onClick={() => {
                    // `canonicalLang` keeps the stored preference the same token the workbench reads.
                    storageSet(WB_LANGUAGE_KEY, canonicalLang(v.language) ?? v.language);
                    switchTo(i);
                    setMenuOpen(false);
                  }}
                >
                  {displayLang(v.language)}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    ) : (
      <span class="wb__lang-pill">{displayLang(variant.language)}</span>
    );

  return (
    <div class="runnable not-prose solution">
      <div class="runnable__bar">
        <span class="wb__eyebrow">
          <span class="wb__prompt">{"✓"}</span> SOLUTION
        </span>
        <span class="wb__actions">
          {langChrome}
          <button class="wb__ghost" title="Load this solution into its language tab on the right" onClick={copyToEditor}>
            Copy to editor
          </button>
        </span>
      </div>
      <div class="runnable__editor" style={`height: ${height}px;`} ref={host}></div>
    </div>
  );
}
