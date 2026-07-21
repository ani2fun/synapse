/**
 * The tour carousel's rotation (A10 tail; port of the stateful half of view/tour.rs — the
 * markup itself SSRs in components/Tour.astro). 7s auto-advance that pauses on hover and
 * wraps; dots + arrows jump; the foot label follows. Toggling is inline `display` — all four
 * slides are already in the DOM, and the viz loader's viewport observer turns slide 4's
 * hidden widget into a lazy wasm load the moment it first shows (display:none never
 * intersects).
 */
import * as log from "../lib/log";

const SLIDE_MS = 7_000;
const EYEBROWS = ["The Library", "Runnable code", "Find your way", "See it work"];

function init(): void {
  const tour = document.getElementById("syn-tour");
  if (!tour) return;
  const slides = Array.from(tour.querySelectorAll<HTMLElement>(".syn-tour__slide"));
  const dots = Array.from(tour.querySelectorAll<HTMLElement>(".syn-tour__dot"));
  const label = tour.querySelector<HTMLElement>(".syn-tour__label");
  const count = slides.length;
  if (count === 0) return;

  let idx = 0;
  let paused = false;

  const show = (next: number): void => {
    idx = ((next % count) + count) % count;
    for (const [i, slide] of slides.entries()) slide.style.display = i === idx ? "" : "none";
    for (const [i, dot] of dots.entries()) dot.classList.toggle("syn-tour__dot--active", i === idx);
    if (label) label.textContent = `0${idx + 1} / 0${count} — ${EYEBROWS[idx] ?? ""}`;
  };

  tour.addEventListener("mouseenter", () => (paused = true));
  tour.addEventListener("mouseleave", () => (paused = false));
  for (const [i, dot] of dots.entries()) dot.addEventListener("click", () => show(i));
  tour.querySelector("[data-tour-prev]")?.addEventListener("click", () => show(idx - 1));
  tour.querySelector("[data-tour-next]")?.addEventListener("click", () => show(idx + 1));

  setInterval(() => {
    if (!paused) show(idx + 1);
  }, SLIDE_MS);
  log.info(`tour: ${count} slides, auto-advance ${SLIDE_MS / 1000}s`);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
