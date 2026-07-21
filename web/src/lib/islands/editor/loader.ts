// ──────────────────────────────────────────────────────────────────
// EDITOR LOADER
// tiny dynamic-import gateway so monaco lands in its own chunk
// ──────────────────────────────────────────────────────────────────
// monaco's editor core + web worker are multi-MB; only lessons with runnable
// blocks pay for them, fetched once and cached.
//
// `mountEditor` takes flat positional args + JS function callbacks rather than
// an options object — that's the wasm-bindgen FFI shape the viz-wasm crate's
// generated bindings call through (see lib/viz-wasm/pkg/viz_wasm.js), so the
// call is folded in here. `setReadOnly` is a small wrapper extension (monaco
// updateOptions) so the ⌘E toggle doesn't recreate the editor and lose cursor
// state.

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
  onSubmit: (() => void) | undefined,
): Promise<EditorHandle> {
  const { createEditor } = await import("./monaco");
  const opts: EditorOptions = { value, language, readOnly, dark, onChange, onRun, onToggleEdit, onSubmit };
  return createEditor(container, opts);
}
