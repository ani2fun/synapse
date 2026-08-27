// `test` comes from ./fixtures: it fails a spec on any uncaught page error, so a boot
// failure names itself instead of surfacing as "element(s) not found".
import { expect, test } from "./fixtures";

/**
 * The popup codebench, ANONYMOUS — so it runs on every push.
 *
 * What this file can prove without a sign-in is the shape of the feature for a signed-out reader,
 * which is precisely that persistence is INERT: the draft is keyed per account, so with no account
 * there is no key, nothing is written, and every open shows the authored fence. That is worth
 * pinning, because the bug being fixed is a reset-on-reopen and the fix is a restore-on-reopen —
 * a mistake in the auth gate would silently start restoring for everyone.
 *
 * The click-path that actually exercises a restore needs an editable buffer, and editing the
 * codebench requires sign-in. It lives in `codebench-authed.spec.ts` behind `E2E_AUTH`.
 */

/** The fixture intro carries a plain ```python fence — runnable, so it grows the button. */
const LESSON = process.env.E2E_CODEBENCH_LESSON ?? "/synapse/learn/smoke/intro";

/**
 * Open state is a CLASS, asserted by count — never `toBeVisible` on `.codebench--open` itself.
 * Its two children are both `position: fixed`, so the wrapper measures 0px tall and Playwright
 * (correctly) calls it invisible while the modal is plainly on screen. `.codebench__frame` is the
 * element with a real box.
 */
const frameOf = (page: import("@playwright/test").Page) => page.locator(".codebench__frame");

async function openTheBench(page: import("@playwright/test").Page) {
  await page.goto(LESSON);
  // `.lesson-body` exists as an empty shell from first paint; the fence group (and its bar) only
  // appear once the markdown island has filled it.
  const tryBtn = page.locator(".fence-group__try").first();
  await expect(tryBtn).toBeVisible({ timeout: 30_000 });
  await tryBtn.click();
  await expect(page.locator(".codebench--open")).toHaveCount(1);
  const frame = frameOf(page);
  await expect(frame).toBeVisible();
  // Monaco is imported on the FIRST open only, so this wait is the cold one.
  await expect(frame.locator(".view-lines")).toContainText("def greet", { timeout: 30_000 });
  return frame;
}

test("a signed-out reader gets the fence back on every open, and nothing is stored", async ({ page }) => {
  const frame = await openTheBench(page);

  // The gate the whole draft feature hangs off: no account, no editing.
  await expect(page.locator(".codebench__signin")).toBeVisible();
  // Nothing differs from the fence, so the draft bar must not be claiming otherwise.
  await expect(page.locator(".codebench__draft")).toHaveCount(0);

  await page.keyboard.press("Escape");
  await expect(page.locator(".codebench--open")).toHaveCount(0);

  // Re-open: a fresh request object every click, which is what used to reset an edited buffer and
  // now goes looking for a draft instead. With no account there is none, so this is the fence.
  await page.locator(".fence-group__try").first().click();
  await expect(page.locator(".codebench--open")).toHaveCount(1);
  await expect(frame.locator(".view-lines")).toContainText("def greet");
  await expect(page.locator(".codebench__draft")).toHaveCount(0);

  const keys = await page.evaluate(() =>
    Object.keys(window.localStorage).filter((k) => k.startsWith("codebench-draft:")),
  );
  expect(keys).toEqual([]);
});

test("Escape leaves the modal mounted rather than tearing Monaco down", async ({ page }) => {
  await openTheBench(page);
  await page.keyboard.press("Escape");

  // The frame stays in the DOM (display:none) so the one editor instance survives — the property
  // the CSS calls "the whole point", and the reason a re-open is instant.
  await expect(page.locator(".codebench")).toHaveCount(1);
  await expect(page.locator(".codebench .monaco-editor")).toHaveCount(1);

  await page.locator(".fence-group__try").first().click();
  await expect(page.locator(".codebench--open")).toHaveCount(1);
  await expect(frameOf(page)).toBeVisible();
  // Still ONE editor — a re-open that remounted Monaco would show two.
  await expect(page.locator(".codebench .monaco-editor")).toHaveCount(1);
});
