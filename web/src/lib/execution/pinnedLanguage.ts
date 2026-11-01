// The reader's pinned code language — the one every code surface opens on.
//
// A lesson's language tabs (`islands/workbench/fenceGroups`), the runnable workbench and the
// solution viewer all read it and all carry a pin that writes it. Switching a tab writes nothing:
// only the pin is a statement about how the reader wants to start.
//
// The stored value is always a CANONICAL token (`canonicalLang`), so a pin set on a ```py fence
// still opens a block whose fence says ```python3. A language the sandbox does not run has no
// canonical token, and cannot be pinned — its block shows no pin.

import { LANGUAGE_PINNED, type LanguagePinned } from "../../islands/workbench/contracts";
import * as log from "../log";
import { LANGUAGE_PIN_KEY, get as storageGet, set as storageSet } from "../storage";
import { canonicalLang } from "./language";

/** The pinned canonical token, or `null` when nothing is pinned (or storage is denied). */
export function pinnedLanguage(): string | null {
  return storageGet(LANGUAGE_PIN_KEY);
}

/** Whether `language` (any alias) is the pinned one. Two unknown languages never match. */
export function isPinned(language: string): boolean {
  const wanted = canonicalLang(language);
  const pinned = pinnedLanguage();
  return wanted !== null && pinned !== null && canonicalLang(pinned) === wanted;
}

/** Pin `language` and tell the page, so every other block switches to it now rather than on the
 *  next load. A language with no canonical token is a no-op. */
export function pinLanguage(language: string): void {
  const canonical = canonicalLang(language);
  if (canonical === null) return;
  storageSet(LANGUAGE_PIN_KEY, canonical);
  log.info(`code will open in ${canonical}`);
  if (typeof window === "undefined") return;
  window.dispatchEvent(
    new CustomEvent<LanguagePinned>(LANGUAGE_PINNED, { detail: { language: canonical } }),
  );
}
