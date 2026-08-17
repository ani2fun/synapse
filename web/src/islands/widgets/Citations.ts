/**
 * INLINE CITATIONS — `<abbr title="…">[i]</abbr>` made reachable without a mouse.
 *
 * Lessons carry their page cites as native `title` attributes (1300+ of them in the system-design
 * book alone). A `title` is a HOVER affordance and nothing else: a touch screen has no hover, so
 * on a phone the cite was not merely hard to read, it was UNREACHABLE — the reader could see the
 * `[i]` marker and had no gesture that would ever reveal what it pointed at.
 *
 * So the native tooltip is retired and replaced by one the page controls. `title` is MOVED to
 * `data-cite` rather than copied: leaving it in place would give a desktop reader two tooltips,
 * the browser's and ours, at different offsets and different delays. This mirrors the choice
 * already made for the workbench toolbar (`.wb__tip[data-tip]`, runnable.css) — same reason.
 *
 * ONE popover for the whole document, positioned on demand. The alternative — a bubble per marker,
 * via `::after` on the anchor — costs nothing at rest but cannot escape the prose column: a cite
 * near the right edge gets clipped, and `overflow` on any ancestor crops it. A single fixed-position
 * node sidesteps both and keeps the DOM flat regardless of how many markers a lesson holds.
 *
 * Listeners are DELEGATED for the same reason. Binding per marker would mean thousands of
 * registrations on a long lesson; three listeners on the document do the same job at any scale.
 */
/** The attribute the cite text lives on after hydration — also the "already done" marker. */
const CITE = "data-cite";
const POP_CLASS = "cite-pop";
const POP_ID = "synapse-cite-pop";
/** Gap between the marker and the bubble, and the minimum breathing room at a viewport edge. */
const GAP = 6;
const EDGE = 8;

let pop: HTMLElement | null = null;
let anchor: HTMLElement | null = null;
/** A pinned bubble survives the pointer leaving; a transient (hover-shown) one does not. */
let pinned = false;

// ─────────────────────────────────────────────────────────────────────────────
// THE SHARED BUBBLE
// ─────────────────────────────────────────────────────────────────────────────

function bubble(): HTMLElement {
  if (pop != null) return pop;
  const el = document.createElement("div");
  el.id = POP_ID;
  el.className = POP_CLASS;
  // `role="tooltip"` + `aria-describedby` on the anchor is what makes the cite reach a screen
  // reader. It is inert to the pointer: the text is passive, and a hoverable bubble would only
  // add flicker as the cursor crosses the gap between marker and bubble.
  el.setAttribute("role", "tooltip");
  el.hidden = true;
  document.body.appendChild(el);
  pop = el;
  return el;
}

/**
 * Place the bubble under its marker, flipping above when the viewport's bottom is closer than the
 * bubble is tall, and clamping horizontally so a cite at either margin stays fully on screen.
 *
 * `position: fixed`, so these are viewport coordinates and no scroll offset enters the maths.
 */
function place(el: HTMLElement, target: HTMLElement): void {
  const at = target.getBoundingClientRect();
  const box = el.getBoundingClientRect();
  const below = at.bottom + GAP;
  const above = at.top - box.height - GAP;
  // Prefer below; go above only when below would run off and above actually fits.
  const top = below + box.height <= window.innerHeight - EDGE || above < EDGE ? below : above;
  const ideal = at.left + at.width / 2 - box.width / 2;
  const max = window.innerWidth - box.width - EDGE;
  el.style.top = `${Math.round(top)}px`;
  el.style.left = `${Math.round(Math.min(Math.max(EDGE, ideal), Math.max(EDGE, max)))}px`;
}

function open(target: HTMLElement, stick: boolean): void {
  const cite = target.getAttribute(CITE);
  if (cite == null || cite === "") return;
  const el = bubble();
  el.textContent = cite;
  el.hidden = false;
  // Measured only once it is laid out — `place` reads the bubble's own height.
  place(el, target);
  target.setAttribute("aria-describedby", POP_ID);
  target.setAttribute("aria-expanded", stick ? "true" : "false");
  anchor = target;
  pinned = stick;
}

function close(): void {
  if (pop != null) pop.hidden = true;
  if (anchor != null) {
    anchor.removeAttribute("aria-describedby");
    anchor.setAttribute("aria-expanded", "false");
  }
  anchor = null;
  pinned = false;
}

/** The marker under an event, or null — one lookup shared by every delegated handler. */
const markerFrom = (event: Event): HTMLElement | null => {
  const node = event.target;
  return node instanceof Element ? node.closest<HTMLElement>(`abbr[${CITE}]`) : null;
};

// ─────────────────────────────────────────────────────────────────────────────
// DELEGATION
// ─────────────────────────────────────────────────────────────────────────────

let wired = false;

function wire(): void {
  if (wired) return;
  wired = true;

  // TAP / CLICK — the whole point of this module, and the only gesture a touch screen has.
  // Toggles, so a second tap on the same marker puts it away without hunting for empty space.
  document.addEventListener("click", (event) => {
    const marker = markerFrom(event);
    if (marker == null) {
      close();
      return;
    }
    event.preventDefault();
    if (anchor === marker && pinned) close();
    else open(marker, true);
  });

  // HOVER — a desktop nicety that restores what the native tooltip used to do. Guarded to FINE
  // pointers: a touch device synthesises pointerenter just before click, which would otherwise
  // open the bubble unpinned and let the click immediately toggle it back shut.
  document.addEventListener("pointerenter", (event) => {
    if ((event as PointerEvent).pointerType !== "mouse") return;
    const marker = markerFrom(event);
    if (marker != null && !pinned) open(marker, false);
  }, true);

  document.addEventListener("pointerleave", (event) => {
    if ((event as PointerEvent).pointerType !== "mouse") return;
    if (markerFrom(event) != null && !pinned) close();
  }, true);

  // KEYBOARD — the marker is focusable, so it must answer the keys a button answers. Escape
  // dismisses without moving focus, which is WCAG 1.4.13's "dismissable".
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      close();
      return;
    }
    if (event.key !== "Enter" && event.key !== " ") return;
    const active = document.activeElement;
    const marker = active instanceof Element ? active.closest<HTMLElement>(`abbr[${CITE}]`) : null;
    if (marker == null) return;
    event.preventDefault(); // Space would scroll the page
    if (anchor === marker && pinned) close();
    else open(marker, true);
  });

  // Focus OPENS but never pins, and that distinction is load-bearing. A tap fires `focusin`
  // BEFORE `click`, so a focus handler that pinned would hand the click a bubble already marked
  // as the reader's own — and the click, doing its job, would toggle it straight back shut. The
  // symptom on a real phone is a marker that responds to a tap by doing nothing at all, which is
  // the very bug this island exists to fix. `click` stays the only authority over the pin.
  document.addEventListener("focusin", (event) => {
    const marker = markerFrom(event);
    if (marker != null) open(marker, false);
    else if (!pinned) close();
  });

  // Scrolling and resizing both invalidate the rect the bubble was placed against, so it has to
  // react — but it must NOT simply dismiss. The browser scrolls a marker into view when it takes
  // focus, and that scroll event lands a frame AFTER `focusin`: closing on it would shut the
  // bubble the Tab key had just opened, every single time. Measured before this was written —
  // one dispatched scroll was enough to hide a bubble opened microseconds earlier.
  //
  // So follow the marker instead, and give up only once it has actually left the viewport. One
  // bubble is open at most, so this is a single rect read per frame while a scroll is in flight.
  let frame = 0;
  const follow = (): void => {
    if (frame !== 0 || anchor == null || pop == null || pop.hidden) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      if (anchor == null || pop == null || pop.hidden) return;
      const at = anchor.getBoundingClientRect();
      if (at.bottom < 0 || at.top > window.innerHeight) close();
      else place(pop, anchor);
    });
  };
  addEventListener("scroll", follow, { passive: true, capture: true });
  addEventListener("resize", follow, { passive: true });
}

// ─────────────────────────────────────────────────────────────────────────────
// HYDRATION
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Convert every `abbr[title]` under `root` into a tappable marker. Idempotent: a marker already
 * carrying `data-cite` has no `title` left to move, so a re-run finds nothing and does nothing.
 *
 * Every `abbr[title]` qualifies, not only the `[i]` citation shape — a spelled-out abbreviation is
 * unreachable on a phone for exactly the same reason, and gains the same fix for free.
 */
export function hydrateCitations(root: ParentNode): number {
  const markers = Array.from(root.querySelectorAll<HTMLElement>("abbr[title]"));
  for (const marker of markers) {
    const cite = marker.getAttribute("title") ?? "";
    marker.removeAttribute("title"); // retire the native tooltip — see the module doc
    marker.setAttribute(CITE, cite);
    // Focusable and announced as a control: without this the marker is inert to the keyboard and
    // a screen reader reads a bare "[i]" with nothing attached to it.
    marker.setAttribute("tabindex", "0");
    marker.setAttribute("role", "button");
    marker.setAttribute("aria-expanded", "false");
    marker.setAttribute("aria-label", `Citation: ${cite}`);
  }
  if (markers.length > 0) wire();
  return markers.length;
}
