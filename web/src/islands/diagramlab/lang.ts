// ──────────────────────────────────────────────────────────────────
// THE TWO DIAGRAM LANGUAGES, AS FACTS
// ──────────────────────────────────────────────────────────────────
// What a lesson page, the lab shell and the splicing all have to agree about: which fence a
// figure came from, which editor edits it, and what the buffer is called. Small enough to be one
// table, and it has to BE one table — a figure whose Edit pill points at `/d2` while the splicer
// looks for mermaid fences would replace someone else's diagram.

import { type Fence, d2Fences, mermaidFences } from "../../lib/markdown/fences";

/** A fence language with an editor behind it. */
export type DiagramLang = "d2" | "mermaid";

interface LangFacts {
  /** Where `/…?lesson=&at=` lives for this language. */
  route: string;
  /** The buffer's extension in the pane header and the Download button. */
  extension: string;
  /** Every fence of this language in a document, in order — what `at` indexes. */
  fences: (markdown: string) => Fence[];
}

const FACTS: Record<DiagramLang, LangFacts> = {
  d2: { route: "/d2", extension: "d2", fences: d2Fences },
  mermaid: { route: "/mermaid", extension: "mmd", fences: mermaidFences },
};

/** Every fence of `lang`, in order. The index into THIS list is a figure's `data-fence-at`. */
export const fencesOfLang = (markdown: string, lang: DiagramLang): Fence[] =>
  FACTS[lang].fences(markdown);

/** The editor page for `lang`. */
export const routeOfLang = (lang: DiagramLang): string => FACTS[lang].route;

/** The file extension a buffer of `lang` downloads as. */
export const extensionOfLang = (lang: DiagramLang): string => FACTS[lang].extension;
