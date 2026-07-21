/**
 * The C4 docs panel (port of client/src/catalog/view/c4_docs.rs — oracle: `C4DocsPanel`,
 * ADR-S032): click a component in an embedded LikeC4 diagram and its tutorial doc — a
 * co-located `_c4-docs/*.md` next to the lesson — slides in from the right. Clicking another
 * component switches context; ✕/Esc close. (RS deviation, on purpose, carried from the Rust
 * port: a fixed right-side panel instead of the oracle's JS grid collapse — the reader column
 * stays put, no inline-!important surgery.)
 */
import { useEffect, useRef, useState } from "preact/hooks";

import { c4Doc } from "../../lib/api/client";
import type { ComponentDoc } from "../../lib/api/client";
import * as log from "../../lib/log";
import { useStore } from "../../lib/store";
import { c4Selected } from "./c4Store";

type DocState = { kind: "loading" } | { kind: "ready"; doc: ComponentDoc } | { kind: "failed"; message: string };

export function C4DocsPanel({ lessonPath }: { lessonPath: string[] }) {
  const selected = useStore(c4Selected);
  const [state, setState] = useState<DocState>({ kind: "loading" });

  // Fetch on every selection change; a stale reply for a superseded selection is dropped.
  useEffect(() => {
    if (selected == null) return;
    setState({ kind: "loading" });
    log.debug(`c4 docs: open ${selected} (lesson ${lessonPath.join("/")})`);
    const current = selected;
    void (async () => {
      try {
        const doc = await c4Doc(current, lessonPath);
        if (c4Selected.get() === current) setState({ kind: "ready", doc });
      } catch (error) {
        if (c4Selected.get() === current) {
          setState({ kind: "failed", message: error instanceof Error ? error.message : String(error) });
        }
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && c4Selected.get() != null) c4Selected.set(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (selected == null) return null;

  return (
    <aside class="c4-docs">
      <div class="c4-docs__head">
        <span class="c4-docs__eyebrow">COMPONENT GUIDE</span>
        <button class="c4-docs__close" aria-label="Close" onClick={() => c4Selected.set(null)}>
          ✕
        </button>
      </div>
      {state.kind === "loading" && <p class="c4-docs__status">Loading the guide…</p>}
      {state.kind === "failed" && (
        <div class="c4-docs__missing">
          <p>
            <b>{selected}</b> has no guide here yet.
          </p>
          <p class="c4-docs__status">{state.message}</p>
        </div>
      )}
      {state.kind === "ready" && <DocBody doc={state.doc} />}
    </aside>
  );
}

function DocBody({ doc }: { doc: ComponentDoc }) {
  const host = useRef<HTMLDivElement>(null);
  const chips = [doc.kind, doc.technology].filter((c): c is string => c != null);

  useEffect(() => {
    const node = host.current;
    if (!node) return;
    let live = true;
    void (async () => {
      try {
        const { renderLesson } = await import("../../lib/markdown/render");
        const html = await renderLesson(doc.body);
        if (live && host.current) host.current.innerHTML = html;
      } catch {
        if (live && host.current) host.current.textContent = doc.body;
      }
    })();
    return () => {
      live = false;
    };
  }, [doc.body]);

  return (
    <>
      {doc.title != null && <h2 class="c4-docs__title">{doc.title}</h2>}
      {chips.length > 0 && (
        <div class="c4-docs__chips">
          {chips.map((chip) => (
            <span class="c4-docs__chip">{chip}</span>
          ))}
        </div>
      )}
      <div class="c4-docs__body" ref={host}></div>
    </>
  );
}
