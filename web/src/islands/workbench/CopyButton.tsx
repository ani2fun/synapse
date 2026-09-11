/**
 * The clipboard button that floats in a `.runnable__editor`'s top-right corner, revealed on hover
 * (`.editor-copy`, viz.css — which carries workbench chrome as well as the viz widgets').
 *
 * Two callers, and they read the same code from different places: the workbench copies the LIVE
 * buffer (whatever the reader has typed), the solution viewer copies the variant on screen. Hence
 * `text` as a getter rather than a value — the workbench's answer changes on every keystroke, and a
 * prop would be a snapshot taken at render.
 *
 * Distinct from the viewer's "Copy to editor", which loads a solution into the workbench tab. This
 * one puts it on the clipboard, for somewhere this app has never heard of.
 */
import { useEffect, useRef, useState } from "preact/hooks";

/** How long the tick stays up. Long enough to read as confirmation, short enough that a second
 *  copy doesn't feel blocked by the first. */
const TICK_MS = 1_400;

const TICK = (
  <svg class="editor-copy__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
    <path d="M20 6 9 17l-5-5"></path>
  </svg>
);

const SHEETS = (
  <svg class="editor-copy__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
    <rect x="8" y="8" width="14" height="14" rx="2" ry="2"></rect>
    <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path>
  </svg>
);

export function CopyButton({ text }: { text: () => string }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | null>(null);

  // An editor evicted (or a pane unmounted) inside the tick window would otherwise leave a timeout
  // to set state on a component that is gone.
  useEffect(() => () => {
    if (timer.current != null) window.clearTimeout(timer.current);
  }, []);

  return (
    <button
      class={`editor-copy${copied ? " editor-copy--done" : ""}`}
      aria-label="Copy code"
      title="Copy code"
      onClick={() => {
        // Optional-chained: a page served over plain HTTP has no `navigator.clipboard` at all, and
        // the button should do nothing rather than throw.
        void navigator.clipboard?.writeText(text());
        setCopied(true);
        if (timer.current != null) window.clearTimeout(timer.current);
        timer.current = window.setTimeout(() => setCopied(false), TICK_MS);
      }}
    >
      {copied ? TICK : SHEETS}
    </button>
  );
}
