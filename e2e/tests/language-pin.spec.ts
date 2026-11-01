// `test` comes from ./fixtures: it fails a spec on any uncaught page error.
import { expect, test } from "./fixtures";

/**
 * The pinned code language on a lesson's language tabs (`fenceGroups.ts`).
 *
 * The fixture intro carries two python + java groups. A group opens on the pinned language, a pin
 * on one group switches every group that offers it, and only the pin survives a reload — a plain
 * tab switch is a visit, not a preference.
 */

const LESSON = "/synapse/learn/smoke/intro";

/** The two-language groups, by the languages they hold — the intro's lone python fence is not one. */
const groups = (page: import("@playwright/test").Page) =>
  page.locator('div.fence-group[data-langs="python,java"]');

const activeTab = (group: import("@playwright/test").Locator) =>
  group.locator(".fence-group__tab--active");

test("language tabs open on the pinned language and follow a pin", async ({ page }) => {
  await page.goto(LESSON);
  const both = groups(page);
  await expect(both).toHaveCount(2);
  const [first, second] = [both.nth(0), both.nth(1)];
  await expect(first.locator(".fence-group__pin")).toBeVisible({ timeout: 30_000 });

  // Nothing pinned: the first tab, and the pin is not pressed.
  await expect(activeTab(first)).toHaveText("Python");
  await expect(first.locator(".fence-group__pin")).toHaveAttribute("aria-pressed", "false");

  // A tab switch alone is not remembered.
  await first.getByRole("button", { name: "Show the Java version" }).click();
  await expect(activeTab(first)).toHaveText("Java");
  await expect(activeTab(second)).toHaveText("Python");
  await page.reload();
  await expect(first.locator(".fence-group__pin")).toBeVisible({ timeout: 30_000 });
  await expect(activeTab(first)).toHaveText("Python");

  // Pin Java on the first group: the second follows at once, and both pins read as pressed.
  await first.getByRole("button", { name: "Show the Java version" }).click();
  await first.locator(".fence-group__pin").click();
  await expect(activeTab(second)).toHaveText("Java");
  await expect(first.locator(".fence-group__pin")).toHaveAttribute("aria-pressed", "true");
  await expect(second.locator(".fence-group__pin")).toHaveAttribute("aria-pressed", "true");

  // The pin survives a reload; the panes on screen are the java ones.
  await page.reload();
  await expect(first.locator(".fence-group__pin")).toBeVisible({ timeout: 30_000 });
  await expect(activeTab(first)).toHaveText("Java");
  await expect(activeTab(second)).toHaveText("Java");
  await expect(second.locator("figure:not(.fence-group__pane--hidden)")).toContainText("System.out.println");

  // Looking at Python on a pinned-Java page shows an unpressed pin.
  await second.getByRole("button", { name: "Show the Python version" }).click();
  await expect(second.locator(".fence-group__pin")).toHaveAttribute("aria-pressed", "false");
});
