// `test` comes from ./fixtures: it fails a spec on any uncaught page error, so a boot failure
// names itself instead of surfacing as "element(s) not found".
import { expect, test } from "./fixtures";

/**
 * `/viz` — the visualisation lab, ANONYMOUS: the wall.
 *
 * The lab runs real programs, so it opens only for a signed-in account the submit gate admits.
 * A signed-out reader gets a sign-in prompt and NOTHING behind it: no Monaco and no wasm, because
 * a bundle that can only answer "sign in" is weight with no use. The lab's own shape is
 * vizlab-authed.spec.ts.
 */

const LAB = "/viz";

test("a signed-out reader meets the wall, not the lab", async ({ page }) => {
  const wasm: string[] = [];
  page.on("request", (request) => {
    if (/viz_wasm.*\.wasm/.test(request.url())) wasm.push(request.url());
  });
  await page.goto(LAB);

  const wall = page.locator('.lab-gate[data-lab-gate="anonymous"]');
  await expect(wall).toBeVisible({ timeout: 30_000 });
  await expect(wall.locator(".lab-gate__title")).toHaveText("Sign in to use the lab");
  await expect(wall.locator("button", { hasText: "Sign in" })).toBeVisible();

  await expect(page.locator(".lab-pane--l")).toHaveCount(0);
  await expect(page.locator(".monaco-editor")).toHaveCount(0);
  expect(wasm).toEqual([]);
});
