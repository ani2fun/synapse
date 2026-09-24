// A PRIVATE book, read by someone on its list — the half only a real Keycloak session can prove.
//
// The page is server-rendered from an anonymous fetch, so a private lesson ships as a shell and
// is fetched and rendered IN the browser once the session settles. What this pins is that the
// shell fills: the title, the prose, the sidebar tree, and the lock on the book in the index —
// and that a signed-in user who is NOT on the list gets told so rather than a blank page.
//
// GATED (set E2E_AUTH=1) and run against a Keycloak-ALLOWLISTED origin: the dev server on :5373.
// `dev-tools/e2e-auth` sets it up; the default `dev-tools/e2e` run skips this file.
import { expect, test } from "./fixtures";

const READER = process.env.E2E_KC_USER ?? "tester";
const READER_PASS = process.env.E2E_KC_PASS ?? "tester";
const OUTSIDER = process.env.E2E_KC_USER2 ?? "test1";
const OUTSIDER_PASS = process.env.E2E_KC_PASS2 ?? "test1";
const LESSON = "/synapse/programming-languages/insight-earned/notes/selection-sort";

async function signIn(page: import("@playwright/test").Page, user: string, pass: string) {
  await page.goto("/");
  await page.locator(".account-chip__signin").click();
  await page.locator("#username").fill(user);
  await page.locator("#password").fill(pass);
  await page.locator("#kc-login").click();
  await expect(page.locator(".account-chip__user")).toHaveText(`@${user}`, { timeout: 30_000 });
}

test.describe("private book — a listed reader, and one who is not", () => {
  test.skip(
    !process.env.E2E_AUTH,
    "set E2E_AUTH=1 with Keycloak up and E2E_BASE_URL a realm-allowlisted origin (e.g. http://localhost:5373)",
  );

  test("the listed reader gets the lesson rendered client-side, with the rail and the lock", async ({ page }) => {
    await signIn(page, READER, READER_PASS);

    const response = await page.goto(LESSON);
    // The server still answers as it answers everyone: the session lives in the browser.
    expect(response?.status()).toBe(401);
    await expect(page.locator("[data-private-title]")).toHaveText("Selection Sort, Rewritten", { timeout: 30_000 });
    await expect(page.locator("[data-private-body]")).toContainText("quokka invariant");
    // The figure in the prose is a private file: an <img> carries no bearer, so the island
    // fetched it itself and handed the browser a blob — and it painted.
    const figure = page.locator('[data-private-body] img[alt="One pass of the scan"]');
    await expect(figure).toBeVisible({ timeout: 10_000 });
    await expect.poll(() => figure.evaluate((el) => (el as HTMLImageElement).naturalWidth)).toBeGreaterThan(0);
    expect(await figure.getAttribute("src")).toMatch(/^blob:/);
    await expect(page.locator("[data-private-sidebar] .reader-sidebar__link--active")).toHaveText("Selection Sort, Rewritten");
    await expect(page.locator("[data-private-sidebar]")).toContainText("Private book");

    // The library landing is rendered from the anonymous index, so the book reaches it only
    // after the session settles — as its own group, ahead of the public grid.
    await page.goto("/");
    const mine = page.locator("#lib-private-group");
    await expect(mine).toBeVisible({ timeout: 30_000 });
    await expect(mine.locator('[data-book-slug="insight-earned"]')).toContainText("Insight Earned");
    await expect(page.locator('[data-book-slug="insight-earned"]')).toHaveCount(1, { timeout: 5_000 });

    // The index the reader was admitted to marks the book, and only that book.
    const index = await page.evaluate(async () => {
      const token = (window as unknown as { __synapseVizToken?: () => string | null }).__synapseVizToken?.();
      const res = await fetch("/api/synapse/index", { headers: token ? { Authorization: `Bearer ${token}` } : {} });
      return res.json();
    });
    const languages = index.entries.find((e: { slug: string }) => e.slug === "programming-languages");
    const insight = languages.entries.find((e: { slug: string }) => e.slug === "insight-earned");
    expect(insight.private).toBe(true);
    expect(languages.entries.find((e: { slug: string }) => e.slug === "java").private).toBeUndefined();
  });

  test("a private problem lesson is the problem page, workbench and all", async ({ page }) => {
    await signIn(page, READER, READER_PASS);
    const response = await page.goto("/synapse/programming-languages/insight-earned/problems/threshold/threshold");
    expect(response?.status()).toBe(401);
    // The shell gives way to the frame the public problem page has, hydrated by the same island.
    await expect(page.locator(".pwb[data-problem] .pwb__title")).toHaveText("Threshold, Rewritten", { timeout: 30_000 });
    await expect(page.locator(".pwb-description")).toContainText("zebra invariant");
    await expect(page.locator(".pcanvas")).toBeVisible({ timeout: 30_000 });
    await page.locator(".pwb__rtab--code").click();
    await expect(page.locator(".pwb__right .runnable")).toBeVisible({ timeout: 30_000 });
    await expect(page.locator(".pwb__right .view-lines")).toContainText("Over", { timeout: 30_000 });
    // The editorial tab carries the private editorial the payload brought along.
    await page.locator(".problem-tab--editorial").click();
    await expect(page.locator('[data-pane="editorial"]')).toContainText("private editorial says so", { timeout: 15_000 });

    // The Contents pill opens the book's drawer here too. The frame replaces the whole shell after
    // the reader island has wired its drawer, so anything that island captured at load is gone.
    await page.locator(".pwb__contents").click();
    const drawer = page.locator(".reader-nav-drawer");
    await expect(drawer).toBeVisible();
    await expect(drawer.locator("a", { hasText: "Threshold, Rewritten" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(drawer).toHaveCount(0);
  });

  test("a signed-in user who is not on the list is told so, and never sees the book listed", async ({ page }) => {
    await signIn(page, OUTSIDER, OUTSIDER_PASS);
    await page.goto(LESSON);
    await expect(page.locator("[data-private-status]")).toContainText("not on its reader list", { timeout: 30_000 });
    await expect(page.locator("body")).not.toContainText("quokka");
    await page.goto("/");
    await expect(page.locator(".account-chip__user")).toHaveText(`@${OUTSIDER}`, { timeout: 30_000 });
    await expect(page.locator("#lib-private-group")).toHaveCount(0);
  });
});
