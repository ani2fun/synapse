import { expect, test } from "@playwright/test";

/**
 * The two stepping figures hold to one contract: their controls carry real accessible names, the
 * card takes arrow keys, and a button that cannot act is disabled rather than silently absorbing
 * the click. `getByRole(… { name })` is the point of these assertions — it resolves through the
 * accessibility tree, so a button whose only content is `‹` passes only when it is labelled.
 *
 * Nothing here waits on the rendered SVG: `D2Slideshow` paints its transport immediately and
 * resolves the multi-MB d2 WASM lazily afterwards, so gating on the figure would be slow and
 * flaky for no extra coverage.
 */

const LESSON = "/synapse/learn/smoke/intro";
const D2 = ".diagram--slides:not(.diagram--frames)";
const FRAMES = ".diagram--frames";

test("the d2 slideshow's steps are named, not read as punctuation", async ({ page }) => {
  await page.goto(LESSON);
  const card = page.locator(D2);
  await expect(card).toHaveCount(1);
  await expect(card.getByRole("button", { name: "Previous slide" })).toBeVisible();
  await expect(card.getByRole("button", { name: "Next slide" })).toBeVisible();
  // The card announces as one unit rather than as loose controls in the prose.
  await expect(card).toHaveAttribute("aria-roledescription", "step-through diagram");
});

test("the d2 slideshow steps, disables its ends, and takes arrow keys", async ({ page }) => {
  await page.goto(LESSON);
  const card = page.locator(D2);
  const label = card.locator(".transport__label");
  await expect(label).toHaveText("1 / 2");
  // At the first slide there is nowhere back to go, and the button says so.
  await expect(card.getByRole("button", { name: "Previous slide" })).toBeDisabled();

  await card.getByRole("button", { name: "Next slide" }).click();
  await expect(label).toHaveText("2 / 2");
  await expect(card.getByRole("button", { name: "Next slide" })).toBeDisabled();

  await card.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(label).toHaveText("1 / 2");
  await page.keyboard.press("ArrowRight");
  await expect(label).toHaveText("2 / 2");
});

test("a disabled step button is visibly dimmed, not just inert", async ({ page }) => {
  await page.goto(LESSON);
  const previous = page.locator(FRAMES).getByRole("button", { name: "Previous frame" });
  await expect(previous).toBeDisabled();
  // Without the :disabled rule this is "1" — the button looks as clickable as its neighbour.
  await expect(previous).toHaveCSS("opacity", "0.45");
});

test("the sidebar collapse controls carry names, not just tooltips", async ({ page }) => {
  await page.goto(LESSON);
  // These sit beside four other SVG-only buttons that were always labelled; they were the gap.
  await expect(page.getByRole("button", { name: "Collapse to a rail" })).toBeAttached();
  await expect(page.getByRole("button", { name: "Hide the sidebar" })).toBeAttached();
});
