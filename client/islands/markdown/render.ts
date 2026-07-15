// ──────────────────────────────────────────────────────────────────
// MARKDOWN RENDER PIPELINE
// unified · remark · rehype · shiki → safe HTML for the lesson reader
// ──────────────────────────────────────────────────────────────────
// Turns trusted first-party lesson markdown (synapse-content, ADR-S010)
// into an HTML string the Laminar client injects via innerHTML. Loaded
// lazily behind loader.ts, so shiki + the plugin graph only land when a
// lesson first opens. Pipeline decisions live in ADR-S015.

import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkRehype from "remark-rehype";
import { defaultHandlers } from "mdast-util-to-hast";
import type { Code, Root, RootContent } from "mdast";
import type { Element, ElementContent, Properties } from "hast";
import { renderD2 } from "./d2";
import rehypeSlug from "rehype-slug";
import rehypePrettyCode from "rehype-pretty-code";
import { createCssVariablesTheme, type ThemeRegistrationRaw } from "shiki";
import rehypeStringify from "rehype-stringify";

// Shiki "CSS variables" theme — token colors are emitted as var(--shiki-*)
// rather than baked hexes, so code recolors with the reader's light/dark
// palette (mapped in tailwind.css). Built once at module load.
const synapseTheme = createCssVariablesTheme({
  name: "synapse",
  variablePrefix: "--shiki-",
  fontStyle: true,
});

// A fence is *runnable* when its meta carries the bare `run` token —
// ```python run . The reader turns these into an interactive editor +
// Run button instead of a static highlighted block (step 11).
const runnableMeta = /(?:^|\s)run(?:$|\s)/;

function isRunnable(node: Code): boolean {
  return node.lang != null && node.meta != null && runnableMeta.test(node.meta);
}

// A run fence may also carry a `viz=<structure>[:<root>]` hint — ```python run viz=array:nums —
// declaring how the traced run should be visualised (step 30). Captured per variant; the shared
// CodeVariant stays hint-free (the client pairs the hint alongside it).
const vizMeta = /(?:^|\s)viz=(\S+)/;

function vizOf(meta: string | null | undefined): string | undefined {
  const m = meta ? vizMeta.exec(meta) : null;
  return m ? m[1] : undefined;
}

// A *solution* fence — ```python solution time=O(n) space=O(1) — is the authored answer to the
// surrounding exercise. Adjacent solution fences (python + java) group exactly like run fences,
// into ONE spoiler-safe placeholder the client reveals on demand (step 16).
const solutionMeta = /(?:^|\s)solution(?:$|\s)/;

function isSolution(node: Code): boolean {
  return node.lang != null && node.meta != null && solutionMeta.test(node.meta);
}

// ── Workbench blocks (step 13) ──────────────────────────────────────
// Adjacent run fences group into ONE workbench placeholder (its language
// variants); a ```testcases JSON fence directly after the group is parsed
// as the block's TestSpec and spliced out of the prose. The shapes mirror
// synapse.shared.execution.{TestSpec, ArgSpec, TestCase} — the authored
// JSON uses `type` (the Scala side maps it to `tpe` at the codec).

type FenceVariant = { lang: string; source: string; viz?: string };
type ArgSpecJson = { id: string; label: string; type: string; placeholder?: string };
type TestCaseJson = { args: Record<string, string>; expected?: string };
type TestSpecJson = { args: ArgSpecJson[]; cases: TestCaseJson[] };

// Group heads consume their following fences during one render pass; the
// handler sees each node once, in order, so a plain marker property works.
const CONSUMED = "__synapseWorkbenchConsumed";

/**
 * Parse + shape-check an authored ```testcases fence. Strict on purpose:
 * an invalid fence renders a visible `.workbench-error` card (and the raw
 * fence below it) so authors see the mistake — never a silently-degraded
 * panel.
 */
function parseTestSpec(raw: string): { spec?: TestSpecJson; error?: string } {
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch (e) {
    return { error: `not valid JSON (${(e as Error).message})` };
  }
  const isStr = (v: unknown): v is string => typeof v === "string";
  const isObj = (v: unknown): v is Record<string, unknown> =>
    typeof v === "object" && v !== null && !Array.isArray(v);
  if (!isObj(data)) return { error: "the top level must be an object with `args` and `cases`" };
  const { args, cases } = data;
  if (!Array.isArray(args)) return { error: "`args` must be an array of {id, label, type}" };
  for (const a of args) {
    const okPlaceholder = !isObj(a) || a.placeholder === undefined || isStr(a.placeholder);
    if (!isObj(a) || !isStr(a.id) || !isStr(a.label) || !isStr(a.type) || !okPlaceholder)
      return { error: "every arg needs string `id`, `label`, `type` (+ optional string `placeholder`)" };
  }
  if (!Array.isArray(cases) || cases.length === 0) return { error: "`cases` must be a non-empty array" };
  for (const c of cases) {
    const okExpected = !isObj(c) || c.expected === undefined || isStr(c.expected);
    if (!isObj(c) || !isObj(c.args) || Object.values(c.args).some((v) => !isStr(v)) || !okExpected)
      return { error: "every case needs an `args` object of strings (+ optional string `expected`)" };
  }
  return { spec: data as unknown as TestSpecJson };
}

/** The visible card an invalid authored block earns (the raw fence still renders after it). */
function authoringError(prefix: string, message: string): Element {
  return {
    type: "element",
    tagName: "div",
    properties: { className: ["workbench-error"] },
    children: [{ type: "text", value: `${prefix} — ${message}.` }],
  };
}

// ── Quiz blocks (step 16) ───────────────────────────────────────────
// A ```quiz fence carries one check-your-understanding card as JSON:
// {prompt, options: [String], answer: String (one of the options), input?}.
// Client-only — it never crosses the wire, so the model lives in the
// client (QuizBlocks), not in shared.

type QuizJson = { prompt: string; options: string[]; answer: string; input?: string };

/** Parse + shape-check a ```quiz fence — strict, so authoring mistakes surface as a visible card. */
function parseQuiz(raw: string): { quiz?: QuizJson; error?: string } {
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch (e) {
    return { error: `not valid JSON (${(e as Error).message})` };
  }
  const isStr = (v: unknown): v is string => typeof v === "string";
  const isObj = (v: unknown): v is Record<string, unknown> =>
    typeof v === "object" && v !== null && !Array.isArray(v);
  if (!isObj(data)) return { error: "the top level must be an object" };
  const { prompt, options, answer, input } = data;
  if (!isStr(prompt) || prompt.trim() === "") return { error: "`prompt` must be a non-empty string" };
  if (!Array.isArray(options) || options.length < 2 || options.some((o) => !isStr(o)))
    return { error: "`options` must be an array of at least two strings" };
  if (!isStr(answer)) return { error: "`answer` must be a string" };
  if (!options.includes(answer)) return { error: "`answer` must be one of the `options`" };
  if (input !== undefined && !isStr(input)) return { error: "`input`, when present, must be a string" };
  return { quiz: data as unknown as QuizJson };
}

// ── D2 diagrams (step 25) ───────────────────────────────────────────
// d2 is rendered at PARSE time (WASM), unlike the client-side mermaid
// island. An async remark transformer rewrites the mdast BEFORE
// remark-rehype: it renders each ```d2 fence to an SVG and replaces it (or
// a run of *consecutive* d2 fences) with a raw-HTML placeholder the client
// mounts — a lone fence → `.d2-block`, a run of ≥2 → a `.d2-slides`
// step-through slideshow. The SVG rides URI-encoded on a data attribute
// (the workbench/quiz pattern), so discovery decodes it purely. A failed
// diagram breaks the run → a visible `.diagram-error` card + the raw fence.
//
// The multi-MB d2 WASM is dynamic-imported inside renderD2, reached only
// when a fence is actually rendered, so diagram-free pages never load it.

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** A raw-HTML mdast node (passes through remark-rehype under allowDangerousHtml). */
function html(value: string): RootContent {
  return { type: "html", value } as RootContent;
}

/**
 * The d2 parse-time pre-pass. Returns a unified transformer that renders + groups d2 fences. It touches
 * nothing (and imports no WASM) when a document has no d2 fence.
 */
function d2Transform() {
  return async (tree: Root): Promise<void> => {
    const kids = tree.children;
    if (!kids.some((n) => n.type === "code" && (n as Code).lang === "d2")) return;

    const out: RootContent[] = [];
    let pending: string[] = []; // consecutive successfully-rendered d2 SVGs
    let salt = 0;

    const flush = () => {
      if (pending.length === 0) return;
      // `d2-slideshow` (mine) is distinct from the authored legacy `<div class="d2-slides">` wrapper the
      // content puts around a slide run — that wrapper is neutralized in CSS (display: contents).
      const value =
        pending.length === 1
          ? `<div class="d2-block" data-svg="${encodeURIComponent(pending[0])}"></div>`
          : `<div class="d2-slideshow" data-slides="${encodeURIComponent(JSON.stringify(pending))}"></div>`;
      out.push(html(value));
      pending = [];
    };

    for (const node of kids) {
      if (node.type === "code" && (node as Code).lang === "d2") {
        try {
          salt += 1;
          pending.push(await renderD2((node as Code).value, `d2-${salt}`));
        } catch (e) {
          flush(); // an error breaks the current run
          out.push(html(`<div class="diagram-error">D2 diagram failed — ${escapeHtml(String((e as Error)?.message ?? e))}.</div>`));
          out.push(node); // the raw fence stays visible for the author
        }
      } else {
        flush();
        out.push(node);
      }
    }
    flush();
    tree.children = out;
  };
}

/**
 * Render one lesson's markdown source to an HTML string.
 *
 * Scope: the GFM core (headings, lists, tables, links, inline code,
 * blockquotes) + fenced code with shiki highlighting — plus **workbench
 * blocks** (steps 11 · 24): a ```lang run fence — or several *adjacent*
 * ones, one per language — becomes one empty
 * `<div class="workbench" data-variants data-spec?>` placeholder
 * (URI-encoded JSON), which the Laminar client discovers and mounts an
 * editor into. A ```testcases JSON fence directly after the group is
 * parsed as the block's test spec (`data-spec`) and spliced out; if its
 * JSON is invalid, a visible `.workbench-error` card renders instead and
 * the raw fence stays. Every other fence is a static highlighted block
 * via the default handler → rehype-pretty-code.
 *
 * **Solution fences** (step 16) — ```lang solution time=O(…) space=O(…) —
 * group adjacently the same way into one spoiler-safe
 * `<div class="solution-block" data-variants data-metas>` placeholder the
 * client reveals on demand. A ```quiz fence becomes a `.quiz-block` card
 * (step 16) and a ```mermaid fence becomes a `.mermaid-block` diagram
 * placeholder (step 24) the client renders as SVG. ```d2 fences are
 * rendered to SVG at parse time and grouped (a run of ≥2 → a slideshow;
 * step 25). A ```viz widget=<structure> fence becomes a `.viz-widget`
 * placeholder carrying its VizCases payload (step 26). An *orphan*
 * ```testcases fence and <details> editorials still pass through as plain
 * highlighted code / raw HTML.
 *
 * Trusted content (ADR-S015): the source is first-party (read from
 * SYNAPSE_ROOT), so raw HTML passes through unmodified — no
 * rehype-sanitize. An untrusted source would render through its own
 * sanitizing seam, not this one.
 *
 * Async because shiki loads grammars on demand; loader.ts already crosses
 * a Promise boundary via dynamic import, so the contract stays stable when
 * async plugins (katex, mermaid) arrive later.
 */
export async function renderLesson(raw: string): Promise<string> {
  const file = await unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(d2Transform) // parse-time d2 → SVG placeholders (before rehype; no-op without a d2 fence)
    .use(remarkRehype, {
      allowDangerousHtml: true,
      handlers: {
        // Run fences → ONE workbench placeholder per adjacent group; a
        // trailing ```testcases fence becomes its spec. All other code
        // defers to the default handler, so shiki still highlights it.
        code(state, node: Code, parent) {
          if ((node as unknown as Record<string, boolean>)[CONSUMED]) return []; // swallowed by its group head

          // Solution fences → ONE spoiler-safe placeholder per adjacent group (step 16).
          if (isSolution(node)) {
            const variants: FenceVariant[] = [{ lang: node.lang!, source: node.value }];
            const metas: string[] = [node.meta ?? ""];
            const kids = parent && "children" in parent ? parent.children : [];
            const at = kids.indexOf(node);
            if (at >= 0) {
              let i = at + 1;
              while (i < kids.length && kids[i].type === "code" && isSolution(kids[i] as Code)) {
                const sib = kids[i] as Code;
                variants.push({ lang: sib.lang!, source: sib.value });
                metas.push(sib.meta ?? "");
                (sib as unknown as Record<string, boolean>)[CONSUMED] = true;
                i += 1;
              }
            }
            return {
              type: "element",
              tagName: "div",
              properties: {
                className: ["solution-block"],
                "data-variants": encodeURIComponent(JSON.stringify(variants)),
                "data-metas": encodeURIComponent(JSON.stringify(metas)),
              },
              children: [],
            };
          }

          // Mermaid fences → a diagram placeholder the client renders as SVG (step 24). The source
          // rides URI-encoded on data-source; mermaid itself is a lazy island (@diagram), so nothing
          // heavy loads here. Every other fence still shiki-highlights via the default handler.
          if (node.lang === "mermaid") {
            return {
              type: "element",
              tagName: "div",
              properties: {
                className: ["mermaid-block"],
                "data-source": encodeURIComponent(node.value),
              },
              children: [],
            };
          }

          // Declarative widget fences → a viz-widget placeholder (step 26). ```viz widget=<structure> carries
          // a VizCases JSON payload; it rides URI-encoded on data-payload, the structure on data-widget. The
          // client decodes + mounts a WidgetHost. Invalid JSON keeps the raw fence under an error card.
          if (node.lang === "viz") {
            const wm = /(?:^|\s)widget=(\S+)/.exec(node.meta ?? "");
            if (wm) {
              const name = wm[1];
              try {
                JSON.parse(node.value);
              } catch (e) {
                return [
                  authoringError(`Widget “${name}” ignored`, `payload is not valid JSON (${(e as Error).message})`),
                  defaultHandlers.code(state, node),
                ].flat();
              }
              return {
                type: "element",
                tagName: "div",
                properties: {
                  className: ["viz-widget"],
                  "data-widget": name,
                  "data-payload": encodeURIComponent(node.value),
                },
                children: [],
              };
            }
          }

          // Quiz fences → one interactive card placeholder each (step 16); invalid JSON keeps the
          // raw fence visible under an authoring-error card, exactly like testcases.
          if (node.lang === "quiz") {
            const parsed = parseQuiz(node.value);
            if (parsed.quiz) {
              return {
                type: "element",
                tagName: "div",
                properties: {
                  className: ["quiz-block"],
                  "data-quiz": encodeURIComponent(JSON.stringify(parsed.quiz)),
                },
                children: [],
              };
            }
            return [authoringError("Quiz ignored", parsed.error!), defaultHandlers.code(state, node)].flat();
          }

          // ```problem → a PRACTICE PROBLEM widget (docs/embedded-practice-problems.md). The fence body is the
          // statement (markdown, → Description tab); it consumes the directly-following ```lang run starter (+
          // language variants), an optional ```testcases judge set, and an optional ```editorial solution
          // (markdown, → Editorial tab), emitting one `.practice-problem` placeholder. Backward compatible:
          // without a ```problem fence, run groups render exactly as before.
          if (node.lang === "problem") {
            const kids = parent && "children" in parent ? parent.children : [];
            const at = kids.indexOf(node);
            const pVariants: FenceVariant[] = [];
            let pSpec: TestSpecJson | undefined;
            let editorial: string | undefined;
            const pExtras: ElementContent[] = [];
            if (at >= 0) {
              let i = at + 1;
              while (i < kids.length && kids[i].type === "code" && isRunnable(kids[i] as Code)) {
                const sib = kids[i] as Code;
                pVariants.push({ lang: sib.lang!, source: sib.value, viz: vizOf(sib.meta) });
                (sib as unknown as Record<string, boolean>)[CONSUMED] = true;
                i += 1;
              }
              if (kids[i]?.type === "code" && (kids[i] as Code).lang === "testcases") {
                const parsed = parseTestSpec((kids[i] as Code).value);
                if (parsed.spec) {
                  pSpec = parsed.spec;
                  (kids[i] as unknown as Record<string, boolean>)[CONSUMED] = true;
                } else {
                  pExtras.push(authoringError("Test cases ignored", parsed.error!)); // raw fence stays visible
                }
                i += 1;
              }
              if (kids[i]?.type === "code" && (kids[i] as Code).lang === "editorial") {
                editorial = (kids[i] as Code).value;
                (kids[i] as unknown as Record<string, boolean>)[CONSUMED] = true;
                i += 1;
              }
            }
            const pProps: Properties = {
              className: ["practice-problem"],
              "data-problem": encodeURIComponent(node.value),
              "data-variants": encodeURIComponent(JSON.stringify(pVariants)),
            };
            if (pSpec) pProps["data-spec"] = encodeURIComponent(JSON.stringify(pSpec));
            if (editorial) pProps["data-editorial"] = encodeURIComponent(editorial);
            const pDiv: Element = { type: "element", tagName: "div", properties: pProps, children: [] };
            return pExtras.length > 0 ? [pDiv, ...pExtras] : pDiv;
          }

          if (!isRunnable(node)) return defaultHandlers.code(state, node);

          // This fence is a group head: collect it + every directly-following run fence as variants.
          const variants: FenceVariant[] = [{ lang: node.lang!, source: node.value, viz: vizOf(node.meta) }];
          let spec: TestSpecJson | undefined;
          const extras: ElementContent[] = [];
          const kids = parent && "children" in parent ? parent.children : [];
          const at = kids.indexOf(node);
          if (at >= 0) {
            let i = at + 1;
            while (i < kids.length && kids[i].type === "code" && isRunnable(kids[i] as Code)) {
              const sib = kids[i] as Code;
              variants.push({ lang: sib.lang!, source: sib.value, viz: vizOf(sib.meta) });
              (sib as unknown as Record<string, boolean>)[CONSUMED] = true;
              i += 1;
            }
            const next = kids[i];
            if (next && next.type === "code" && (next as Code).lang === "testcases") {
              const parsed = parseTestSpec((next as Code).value);
              if (parsed.spec) {
                spec = parsed.spec;
                (next as unknown as Record<string, boolean>)[CONSUMED] = true; // spliced into data-spec
              } else {
                extras.push(authoringError("Test cases ignored", parsed.error!)); // the raw fence stays visible below
              }
            }
          }

          const properties: Properties = {
            className: ["workbench"],
            "data-variants": encodeURIComponent(JSON.stringify(variants)),
          };
          if (spec) properties["data-spec"] = encodeURIComponent(JSON.stringify(spec));
          const div: Element = { type: "element", tagName: "div", properties, children: [] };
          return extras.length > 0 ? [div, ...extras] : div;
        },
      },
    })
    .use(rehypeSlug)
    .use(rehypePrettyCode, {
      // createCssVariablesTheme() returns shiki's ThemeRegistration, which doesn't
      // structurally satisfy rehype-pretty-code's narrower `theme` type — a .d.ts
      // strictness gap, not a runtime issue (it IS a valid shiki theme object).
      theme: synapseTheme as ThemeRegistrationRaw,
      keepBackground: true,
      defaultLang: "plaintext", // un-tagged fences still render
      bypassInlineCode: true, // inline `code` stays a plain chip, not shiki
    })
    .use(rehypeStringify, { allowDangerousHtml: true })
    .process(raw);
  return String(file);
}
