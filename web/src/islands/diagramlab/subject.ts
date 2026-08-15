// ──────────────────────────────────────────────────────────────────
// THE DIAGRAM AN EDITOR WAS OPENED ON
// ──────────────────────────────────────────────────────────────────
// `?lesson=&at=&count=` — written by the Edit pill on a rendered figure, read here. Absent means
// a blank draft, which is what a diagram editor is without one.

/** The diagram this page was opened on. */
export interface Subject {
  lessonPath: string;
  /** Which fence OF THIS EDITOR'S LANGUAGE, and how many it covers (a d2 slideshow run is
   *  several; a mermaid figure is always one). */
  at: number;
  count: number;
}

export function subjectFromUrl(search: string): Subject | null {
  const params = new URLSearchParams(search);
  const lessonPath = params.get("lesson");
  const at = Number(params.get("at"));
  if (lessonPath == null || lessonPath === "" || !Number.isInteger(at) || at < 0) return null;
  const count = Number(params.get("count"));
  return { lessonPath, at, count: Number.isInteger(count) && count > 0 ? count : 1 };
}
