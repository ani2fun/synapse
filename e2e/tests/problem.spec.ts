import { test, expect } from "./fixtures";

/**
 * The problem-page smoke: the two-pane workbench frame is a page kind of its own — SSR frame
 * plus the extraction island — and until this spec, nothing in CI opened one.
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

  // The SSR frame: crumbs visible (not clipped under the fixed header — a past padding
  // regression), the four tabs, the docked nav.
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

  // The page itself must not scroll — the panes own all scrolling.
  const overflow = await page.evaluate(
    () => document.documentElement.scrollHeight - window.innerHeight,
  );
  expect(overflow).toBeLessThanOrEqual(0);
});

test("the editorial tab mounts its stepper", async ({ page }) => {
  await page.goto(PROBLEM);
  await expect(page.locator(".pwb__right .runnable")).toBeVisible();

  await page.locator(".problem-tab--editorial").click();
  // The editorial stepper island renders on first open: the pane scroller plus at least one
  // Jump pill.
  await expect(page.locator(".pwb-escroll")).toBeVisible();
  await expect(page.getByRole("button", { name: /intuition/i }).first()).toBeVisible();
});

/**
 * The solution is its own document in the index, and its result row has to keep the promise it
 * makes. "comparison" is written in the editorial and NOWHERE else in the fixture library — not
 * in the problem statement, not in a title — so a hit on it can only be the walkthrough.
 */
test("a search hit on a solution opens the editorial, not the problem statement", async ({ page }) => {
  await page.goto("/");
  await page.locator(".header__search").click();
  await page.locator(".cmdk__input").fill("comparison");

  // `.first()` because the editorial says "comparison" twice and every occurrence is marked.
  const row = page.locator(".cmdk__result").first();
  await expect(row.locator("mark").first()).toHaveText("comparison");
  // The chip is the spoiler warning: a reader must be able to see this is the answer before
  // clicking, because the row's title is the PROBLEM'S.
  await expect(row.locator(".cmdk__result-kind")).toHaveText("Solution");

  await row.click();
  await expect(page).toHaveURL(`${PROBLEM}#editorial`);
  // Landing on the page is not the promise — landing on the TAB is.
  await expect(page.locator(".problem-tab--editorial")).toHaveClass(/problem-tab--active/);
  await expect(page.locator(".pwb-escroll")).toBeVisible();
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
