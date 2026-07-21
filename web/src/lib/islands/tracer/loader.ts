// ──────────────────────────────────────────────────────────────────
// TRACER LOADER
// tiny dynamic-import gateway so the harness string lands lazily
// ──────────────────────────────────────────────────────────────────
// Same trick as @editor/loader: the viz-wasm crate's generated bindings import
// THIS module via @tracer/loader, and the dynamic import()s below keep each
// harness string (python.ts + python-harness.py; java.ts + java-harness.java)
// off the initial bundle — each loaded once when the reader first Visualises
// that language.

import type { wrapPython } from "./python";
import type { wrapJava } from "./java";

type WrapPythonFn = typeof wrapPython;
type WrapJavaFn = typeof wrapJava;

let cachedPython: Promise<WrapPythonFn> | null = null;
let cachedJava: Promise<WrapJavaFn> | null = null;

/** Lazily load the python tracer wrap on first call; the resolved fn is cached. */
export function loadWrapPython(): Promise<WrapPythonFn> {
  if (!cachedPython) cachedPython = import("./python").then((m) => m.wrapPython);
  return cachedPython;
}

/** Lazily load the java tracer wrap on first call; the resolved fn is cached. */
export function loadWrapJava(): Promise<WrapJavaFn> {
  if (!cachedJava) cachedJava = import("./java").then((m) => m.wrapJava);
  return cachedJava;
}
