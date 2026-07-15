// ──────────────────────────────────────────────────────────────────
// EDITOR LOADER
// tiny dynamic-import gateway so monaco lands in its own chunk
// ──────────────────────────────────────────────────────────────────
// monaco's editor core + web worker are multi-MB; only lessons with runnable
// blocks pay for them, fetched once and cached (the oracle's loader pattern).
//
// Oracle deviation, on purpose (same as @markdown): the oracle exports
// loadCreateEditor() and Scala invokes the fn with an options OBJECT of
// callbacks; flat args + js functions are the friendlier wasm-bindgen FFI
// shape, so the call is folded in here. `setReadOnly` is a small wrapper
// extension (monaco updateOptions) so the ⌘E toggle doesn't recreate the
// editor and lose cursor state.

import type { EditorHandle, EditorOptions } from "./monaco";

export async function mountEditor(
  container: HTMLElement,
  value: string,
  language: string,
  readOnly: boolean,
  dark: boolean,
  onChange: (value: string) => void,
  onRun: () => void,
  onToggleEdit: () => void,
): Promise<EditorHandle> {
  const { createEditor } = await import("./monaco");
  const opts: EditorOptions = { value, language, readOnly, dark, onChange, onRun, onToggleEdit };
  return createEditor(container, opts);
}
