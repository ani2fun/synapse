import { test, expect } from "./fixtures";

/**
 * The problem-page smoke (migration step A12): the two-pane workbench frame is a page kind of
 * its own — SSR frame + the extraction island — and until this spec, nothing in CI opened one.
 * The fixture problem (`learn/smoke/problems/threshold/threshold`) is ours and stable, so the
 * path is hardcoded rather than discovered — a rename here is a deliberate edit, not drift.
 *
 * Deliberately NOT exercised: Run/Submit (go-judge is not part of this suite's stack) and the
 * signed-in gates (Keycloak is not either). The fixtures' pageerror guard rides along, so a
 * hydration crash anywhere on the page fails the spec even where no assertion looks.
 */

const PROBLEM = "/synapse/learn/smoke/problems/threshold/threshold";

test("the problem page renders its frame and extracts the workbench", async ({ page }) => {
  await page.goto(PROBLEM);

  // The SSR frame: crumbs visible (not under the fixed header — the A07 padding regression),
  // the four tabs, the docked nav.
  const crumbs = page.locator(".pwb__crumbs");
  await expect(crumbs).toBeVisible();
  expect((await crumbs.boundingBox())?.y ?? 0).toBeGreaterThan(60);
  await expect(page.locator(".problem-tab")).toHaveCount(4);
  await expect(page.locator(".pwb__nav")).toBeVisible();

  // The extraction island: the FIRST description workbench lands in the right pane, with the
  // toolbar's Run button and the tests panel's case chips.
  const right = page.locator(".pwb__right");
  await expect(right.locator(".runnable")).toBeVisible();
  await expect(right.locator(".runnable__run")).toBeVisible();
  await expect(right.locator(".wb__chip").first()).toBeVisible();

  // The page itself must not scroll — the panes own all scrolling (step 37's contract).
  const overflow = await page.evaluate(
    () => document.documentElement.scrollHeight - window.innerHeight,
  );
  expect(overflow).toBeLessThanOrEqual(0);
});

test("the editorial tab mounts its stepper", async ({ page }) => {
  await page.goto(PROBLEM);
  await expect(page.locator(".pwb__right .runnable")).toBeVisible();

  await page.locator(".problem-tab--editorial").click();
  // The A08 stepper island renders on first open: the pane scroller plus at least one Jump pill.
  await expect(page.locator(".pwb-escroll")).toBeVisible();
  await expect(page.getByRole("button", { name: /intuition/i }).first()).toBeVisible();
});

test("the contents pill opens the book drawer", async ({ page }) => {
  await page.goto(PROBLEM);
  await expect(page.locator(".pwb__right .runnable")).toBeVisible();

  await page.locator(".pwb__contents").click();
  // The drawer must be genuinely visible (it once mounted into display:none at desktop width).
  const drawer = page.locator(".reader-nav-drawer");
  await expect(drawer).toBeVisible();
  await expect(drawer.locator("a").first()).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(drawer).toHaveCount(0);
});
