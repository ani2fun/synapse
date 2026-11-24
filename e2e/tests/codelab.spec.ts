// `test` comes from ./fixtures: it fails a spec on any uncaught page error, so a boot failure
// names itself instead of surfacing as "element(s) not found".
import { expect, test } from "./fixtures";

/**
 * `/code` — the code lab, ANONYMOUS: the wall.
 *
 * The lab runs real programs in the sandbox, so it opens only for a signed-in account the submit
 * gate admits. A signed-out reader gets a sign-in prompt and NOTHING behind it: no tabs and no
 * Monaco. The lab's own shape is codelab-authed.spec.ts.
 */

const LAB = "/code";

test("a signed-out reader meets the wall, not the lab", async ({ page }) => {
  await page.goto(LAB);

  const wall = page.locator('.lab-gate[data-lab-gate="anonymous"]');
  await expect(wall).toBeVisible({ timeout: 30_000 });
  await expect(wall.locator(".lab-gate__title")).toHaveText("Sign in to use the lab");
  await expect(wall.locator("button", { hasText: "Sign in" })).toBeVisible();

  await expect(page.locator(".clab-tabs")).toHaveCount(0);
  await expect(page.locator("[data-codelab-editor]")).toHaveCount(0);
  await expect(page.locator(".monaco-editor")).toHaveCount(0);
});

test("a share link on a signed-out visit still meets the wall", async ({ page }) => {
  // The fragment never reaches the server; what matters is that the page does not open it
  // behind the reader's back before they can use it.
  await page.goto(`${LAB}#s=not-a-real-link`);
  await expect(page.locator('.lab-gate[data-lab-gate="anonymous"]')).toBeVisible({ timeout: 30_000 });
  await expect(page.locator(".monaco-editor")).toHaveCount(0);
});
