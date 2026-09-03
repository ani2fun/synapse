// The signed-in half of the Think canvas: save an entry, find it in the Saved table, read it back,
// and delete it. This is the path the hermetic suite cannot reach — the entries are the ACCOUNT's,
// stored in Postgres the way submissions are, so nothing about them exists without a real sign-in.
//
// GATED (set E2E_AUTH=1) and run against a Keycloak-ALLOWLISTED origin: the dev server on :5373 (a
// silent port bump 403s the silent-SSO iframe — the scar the repo records). `dev-tools/e2e-auth`
// sets it up; the default `dev-tools/e2e` run skips this file.
//
// Self-cleaning: it deletes the entry it created, so re-runs neither accumulate rows nor depend on
// a clean database. The Problem area carries a run-unique marker, so a leftover row from an
// interrupted run can never be mistaken for this run's.
import { expect, test } from "./fixtures";

const USER = process.env.E2E_KC_USER ?? "tester";
const PASS = process.env.E2E_KC_PASS ?? "tester";
const PROBLEM = process.env.E2E_AUTH_PROBLEM ?? "/synapse/learn/smoke/problems/threshold/threshold";

test.describe("signed-in design canvas — save, read back, delete", () => {
  test.skip(
    !process.env.E2E_AUTH,
    "set E2E_AUTH=1 with Keycloak up and E2E_BASE_URL a realm-allowlisted origin (e.g. http://localhost:5373)",
  );

  test("sign in → fill the canvas → save → view → delete", async ({ page }) => {
    const marker = `e2e canvas ${Date.now()}`;

    // ── sign in through Keycloak (keycloak-js redirects to the realm login form) ──
    await page.goto("/");
    await page.locator(".account-chip__signin").click();
    await page.locator("#username").fill(USER);
    await page.locator("#password").fill(PASS);
    await page.locator("#kc-login").click();
    await expect(page.locator(".account-chip__user")).toHaveText(`@${USER}`, { timeout: 30_000 });

    // ── the problem's Think pane ──
    await page.goto(PROBLEM);
    await page.locator(".pwb__rtab--think").click();
    const canvas = page.locator(".pcanvas");
    await expect(canvas).toBeVisible();

    // ── fill enough of it to be worth keeping: an area, and an idea with a complexity ──
    await page.locator('.pcanvas__field[aria-label="Problem"]').fill(marker);
    await page.locator('.pcanvas__field[aria-label="Constraints"]').fill("· max N — 1e4");
    const firstIdea = page.locator(".pcanvas__idea").first();
    await firstIdea.locator('[aria-label="Idea description"]').fill("scan once, keep a counter");
    await firstIdea.locator('[aria-label="Time complexity"]').fill("O(n)");
    await firstIdea.locator('[aria-label="Space complexity"]').fill("O(1)");
    await expect(page.locator(".pcanvas__meter-label")).toHaveText("3 / 8");

    // ── save ──
    await page.locator(".pcanvas__btn", { hasText: "Save entry" }).click();
    await expect(page.locator(".pcanvas__toast")).toContainText("Entry saved");
    // The tab's own count is the fastest honest signal that a row landed.
    await expect(page.locator(".pcanvas__seg-btn", { hasText: "Saved" })).toContainText("Saved · 1");

    // ── the Saved table: the row DERIVES its title, areas and best complexity from the body ──
    await page.locator(".pcanvas__seg-btn", { hasText: "Saved" }).click();
    const row = page.locator(".pcanvas__table tbody tr", { hasText: marker });
    await expect(row).toHaveCount(1);
    await expect(row).toContainText("3 / 8");
    await expect(row).toContainText("O(n) / O(1)");

    // ── view it: the form comes back filled, frozen, and says so ──
    await row.locator('[aria-label="View entry"]').click();
    await expect(page.locator(".pcanvas__viewing-title")).toHaveText(marker);
    await expect(page.locator('.pcanvas__field[aria-label="Problem"]')).toHaveValue(marker);
    await expect(page.locator('.pcanvas__field[aria-label="Problem"]')).toHaveAttribute("readonly", "");

    // ── back to the draft: reading an entry must not have overwritten what was being written ──
    await page.locator(".pcanvas__btn", { hasText: "Back to draft" }).click();
    await expect(page.locator(".pcanvas__viewing-title")).toHaveCount(0);
    await expect(page.locator('.pcanvas__field[aria-label="Problem"]')).toHaveValue(marker);

    // ── the entry SURVIVES a reload: it is the account's, not the page's ──
    await page.reload();
    await page.locator(".pwb__rtab--think").click();
    await page.locator(".pcanvas__seg-btn", { hasText: "Saved" }).click();
    const persisted = page.locator(".pcanvas__table tbody tr", { hasText: marker });
    await expect(persisted).toHaveCount(1);

    // ── delete: the row goes, and stays gone across a reload ──
    await persisted.locator('[aria-label="Delete entry"]').click();
    await expect(page.locator(".pcanvas__toast")).toContainText("Entry deleted");
    await expect(page.locator(".pcanvas__table tbody tr", { hasText: marker })).toHaveCount(0);

    await page.reload();
    await page.locator(".pwb__rtab--think").click();
    await page.locator(".pcanvas__seg-btn", { hasText: "Saved" }).click();
    await expect(page.locator(".pcanvas__table tbody tr", { hasText: marker })).toHaveCount(0);
  });
});
