/**
 * The pin beside a language picker: "open code in THIS language by default". Two callers — the
 * workbench and the solution viewer — and a lesson's plain language tabs draw the same control in
 * vanilla TS (`fenceGroups.ts`).
 *
 * It repaints itself on LANGUAGE_PINNED, so a pin set on another block un-presses this one without
 * the parent re-rendering for it. Switching the parent's tab is the parent's job; this only reads
 * and writes the preference.
 */
import { useEffect, useState } from "preact/hooks";

import { PIN_PATHS } from "../../lib/catalog/problemFrame";
import { displayLang } from "../../lib/execution/blocks";
import { isPinned, pinLanguage } from "../../lib/execution/pinnedLanguage";
import { LANGUAGE_PINNED } from "./contracts";

export function LanguagePin({ language }: { language: string }) {
  const [, setTick] = useState(0);

  useEffect(() => {
    const repaint = () => setTick((n) => n + 1);
    window.addEventListener(LANGUAGE_PINNED, repaint);
    return () => window.removeEventListener(LANGUAGE_PINNED, repaint);
  }, []);

  // Pressed is a READOUT of the setting, not click feedback: the reader learns what code opens in
  // without having to click to find out.
  const on = isPinned(language);
  const hint = on
    ? `Code opens in ${displayLang(language)} by default`
    : `Open code in ${displayLang(language)} by default`;
  return (
    <button
      class={`wb__pin${on ? " wb__pin--on" : ""}`}
      type="button"
      aria-pressed={on}
      aria-label={hint}
      title={hint}
      onClick={() => pinLanguage(language)}
    >
      <svg
        class="wb__pin-ic"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: PIN_PATHS }}
      ></svg>
    </button>
  );
}
