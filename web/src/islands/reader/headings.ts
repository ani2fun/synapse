// The lesson's own headings, and the one way to jump to one — shared by the reader chrome's
// outline (`islands/chrome`) and the reader menu's scrubber (`islands/reader/scrubber`), so both
// agree about which headings exist and where a jump lands.

/** A harvested prose heading (h2/h3 with an id — rehype-slug mints them). */
export interface Heading {
  id: string;
  text: string;
  level: number;
}

/** How far below the viewport's top a heading lands after a jump: clear of the fixed header. */
export const HEADER_OFFSET = 80;

/** Build an SVG icon element from its inner path markup. */
export function icon(cls: string, inner: string): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", cls);
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  svg.innerHTML = inner;
  return svg;
}

/** A heading's distance from the top of the DOCUMENT, or null when it is not on the page. A
 *  heading inside a closed `<details>` measures all zeros, which is one reason harvesting skips
 *  them. */
export function documentTop(id: string): number | null {
  const el = document.getElementById(id);
  if (!el) return null;
  return el.getBoundingClientRect().top + window.scrollY;
}

/** Jump to a heading, offset for the fixed header. */
export function scrollToHeading(id: string): void {
  const top = documentTop(id);
  if (top === null) return;
  window.scrollTo(0, top - HEADER_OFFSET);
}

/** Harvest h2[id]/h3[id] from the rendered prose (the leading h1 is the page title and has no
 *  id).
 *
 *  A heading inside a `<details>` is skipped. Two reasons, and they agree: the outline should
 *  describe what the page is showing, and a collapsed section is not showing eleven components —
 *  it is showing one line the reader can open. And `scrollToHeading` measures a bounding rect,
 *  which is all zeros while the ancestor is closed, so such a row would not even scroll where it
 *  claims. The section's own heading sits OUTSIDE the `<details>` and keeps its row. */
export function harvestHeadings(body: Element): Heading[] {
  const out: Heading[] = [];
  body.querySelectorAll<HTMLElement>("h2[id], h3[id]").forEach((el) => {
    const id = el.getAttribute("id");
    if (!id || el.closest("details") !== null) return;
    out.push({ id, text: el.textContent ?? "", level: el.tagName.toLowerCase() === "h3" ? 3 : 2 });
  });
  return out;
}
