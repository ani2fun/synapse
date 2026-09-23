// The signed-in workbench: the buffer is EDITABLE from the first keystroke, with no Edit step in
// between. What the hermetic suite pins is the anonymous half (a disabled Edit, a read-only buffer,
// see problem.spec.ts); what only a real sign-in can prove is that the lock lifts by itself.
//
// GATED (set E2E_AUTH=1) and run against a Keycloak-ALLOWLISTED origin: the dev server on :5373 (a
// silent port bump 403s the silent-SSO iframe — the scar the repo records). `dev-tools/e2e-auth`
// sets it up; the default `dev-tools/e2e` run skips this file.
//
// Self-cleaning: Reset puts the authored source back and nothing here reaches the server — the
// edited buffer is page-local, which is the whole point of the lock never having been a boundary.
import { expect, test } from "./fixtures";

const USER = process.env.E2E_KC_USER ?? "tester";
const PASS = process.env.E2E_KC_PASS ?? "tester";
const PROBLEM = process.env.E2E_AUTH_PROBLEM ?? "/synapse/learn/smoke/problems/threshold/threshold";

test.describe("signed-in workbench — editable without an Edit step", () => {
  test.skip(
    !process.env.E2E_AUTH,
    "set E2E_AUTH=1 with Keycloak up and E2E_BASE_URL a realm-allowlisted origin (e.g. http://localhost:5373)",
  );

  test("sign in → the editor takes typing, Reset restores the starter", async ({ page }) => {
    // ── sign in through Keycloak (keycloak-js redirects to the realm login form) ──
    await page.goto("/");
    await page.locator(".account-chip__signin").click();
    await page.locator("#username").fill(USER);
    await page.locator("#password").fill(PASS);
    await page.locator("#kc-login").click();
    await expect(page.locator(".account-chip__user")).toHaveText(`@${USER}`, { timeout: 30_000 });

    // ── the problem's Code pane ──
    await page.goto(PROBLEM);
    await expect(page.locator(".pcanvas")).toBeVisible();
    await page.locator(".pwb__rtab--code").click();
    const bench = page.locator(".pwb__right .runnable");
    await expect(bench).toBeVisible();

    // No Edit button for a reader who can already edit; Reset is there and idle.
    await expect(bench.locator(".wb__actions .wb__ghost", { hasText: "Edit" })).toHaveCount(0);
    const reset = bench.locator('button[aria-label="Reset"]');
    await expect(reset).toBeVisible();
    await expect(reset).toBeDisabled();

    // Typing lands — the buffer was never locked.
    const lines = bench.locator(".view-lines");
    await expect(lines).toBeVisible({ timeout: 30_000 });
    await lines.click();
    await page.keyboard.press("Control+Home");
    await page.keyboard.type("# e2e typed here\n");
    await expect(lines).toContainText("# e2e typed here");
    await expect(reset).toBeEnabled();

    // Reset: the authored starter is back, and Reset goes idle with it.
    await reset.click();
    await expect(lines).not.toContainText("# e2e typed here");
    await expect(reset).toBeDisabled();
  });
});
