import { expect, test } from "@playwright/test";

/**
 * Frame runs, end to end. A run of `<caption> — frame i of N` images is one animation, so the
 * reader gets ONE figure with a transport rather than N stacked pictures — and, the part only a
 * browser can prove, the page fetches one frame instead of the whole run. The fixture lesson
 * carries all three shapes the pipeline has to tell apart: a captioned lone still, a three-frame
 * run, and a marker with nothing under it.
 */

const LESSON = "/synapse/learn/smoke/intro";
const CARD = ".diagram--frames";

test("a run becomes one stepping figure, and only one frame is fetched", async ({ page }) => {
  const framePngs: string[] = [];
  page.on("request", (request) => {
    const url = request.url();
    if (/\/media\/smoke\/frames\/.*\.png$/.test(url)) framePngs.push(url.split("/").pop()!);
  });

  await page.goto(LESSON);
  const card = page.locator(CARD);
  await expect(card).toHaveCount(1);
  // One <img>, not three: the whole reason the widget exists.
  await expect(card.locator(".diagram__figure img")).toHaveCount(1);
  await expect(card.locator(".diagram__caption")).toHaveText("Three stills the reader steps through");
  await expect(card.locator(".transport__label")).toHaveText("1 / 3");

  // The run's later frames are not on the wire before the reader asks for them. step-01 is the
  // lone still AND the run's first frame, so the ceiling is the first frame plus one warmed
  // neighbour — never the whole run.
  await expect
    .poll(() => framePngs.filter((name) => name === "step-03.png").length)
    .toBe(0);
});

test("the transport steps the figure, forwards and back", async ({ page }) => {
  await page.goto(LESSON);
  const card = page.locator(CARD);
  const shown = card.locator(".diagram__figure img");
  await expect(shown).toHaveAttribute("src", /step-01\.png$/);

  await card.getByRole("button", { name: "Next frame" }).click();
  await expect(card.locator(".transport__label")).toHaveText("2 / 3");
  await expect(shown).toHaveAttribute("src", /step-02\.png$/);
  await expect(shown).toHaveAttribute("alt", "Three stills the reader steps through — frame 2 of 3");

  await card.getByRole("button", { name: "Previous frame" }).click();
  await expect(card.locator(".transport__label")).toHaveText("1 / 3");
  await expect(shown).toHaveAttribute("src", /step-01\.png$/);
  // At the ends the transport says so rather than silently doing nothing.
  await expect(card.getByRole("button", { name: "Previous frame" })).toBeDisabled();
});

test("arrow keys step the focused card", async ({ page }) => {
  await page.goto(LESSON);
  const card = page.locator(CARD);
  await card.focus();
  await page.keyboard.press("ArrowRight");
  await expect(card.locator(".transport__label")).toHaveText("2 / 3");
  await page.keyboard.press("ArrowLeft");
  await expect(card.locator(".transport__label")).toHaveText("1 / 3");
});

test("Enlarge opens the current frame and Esc closes it", async ({ page }) => {
  await page.goto(LESSON);
  const card = page.locator(CARD);
  await card.getByRole("button", { name: "Next frame" }).click();
  await expect(card.locator(".diagram__figure img")).toHaveAttribute("src", /step-02\.png$/);

  await card.locator(".diagram__zoom").click();
  const scrim = page.locator(".diagram-zoom-scrim");
  await expect(scrim).toBeVisible();
  await expect(scrim.locator(".diagram-zoom__figure img")).toHaveAttribute("src", /step-02\.png$/);

  await page.keyboard.press("Escape");
  await expect(scrim).toHaveCount(0);
});

test("a lone still is captioned, and a marker with no image stays prose", async ({ page }) => {
  await page.goto(LESSON);
  const figure = page.locator(".prose-figure");
  await expect(figure).toHaveCount(1);
  await expect(figure.locator("figcaption")).toHaveText("A single still, captioned by the line above it");
  await expect(figure.locator("img")).toHaveAttribute("loading", "lazy");

  const body = page.locator(".lesson-body");
  // The consumed markers are gone from the prose…
  await expect(body).not.toContainText("Interactive Diagram (3 frames)");
  // …and the one that captions nothing is untouched, because it is the only copy of its sentence.
  await expect(body).toContainText("// Diagram: A marker with no image under it, which must stay readable prose");
});
