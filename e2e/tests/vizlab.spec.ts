// `test` comes from ./fixtures: it fails a spec on any uncaught page error, so a boot failure
// names itself instead of surfacing as "element(s) not found".
import { expect, test } from "./fixtures";

/**
 * `/viz` — the visualisation lab, ANONYMOUS.
 *
 * What this suite can prove is the SHAPE: the two panes, an editable workbench, the structure
 * picker fed by the wasm's own vocabulary, and the export control. It deliberately never presses
 * Trace — that runs real code in the sandbox, and no spec in this suite does (see
 * codebench.spec.ts, which asserts the popup's shape for the same reason).
 *
 * The picker is the load-bearing assertion. It renders EMPTY until the wasm bundle lands and
 * hands over `viz_structures()`, so a populated one proves three things at once: the lazy loader
 * ran, the bundle booted, and the panel provider is installed. A page that shipped without the
 * wasm would still render everything else here.
 */

const LAB = "/viz";

test("the lab opens on two panes with an editable workbench", async ({ page }) => {
  await page.goto(LAB);

  await expect(page.locator(".lab-pane--l")).toBeVisible();
  await expect(page.locator(".lab-pane--r")).toBeVisible();
  await expect(page.locator(".lab-split")).toBeVisible();

  // Monaco is lazy; the starter is what proves it arrived with the right buffer.
  await expect(page.locator(".lab-pane--r .view-lines")).toContainText("arr = [5, 2, 8, 1, 9, 3]", {
    timeout: 30_000,
  });

  // A playground has no authored source to protect, so there is no Edit gate to sign in for —
  // without that, an anonymous reader could not type a character.
  await expect(page.locator(".lab-pane--r .runnable__bar")).not.toContainText("Edit");
  await expect(page.locator(".runnable__run")).toBeEnabled();
});

test("the structure picker is fed by the crate's own vocabulary", async ({ page }) => {
  await page.goto(LAB);
  const picker = page.locator(".vlab__select");
  // Populated only once the wasm has booted and installed the panel provider.
  await expect(picker.locator("option")).toHaveCount(17, { timeout: 30_000 });
  await expect(picker).toHaveValue("array");
  await expect(picker.locator("option", { hasText: "union-find" })).toHaveCount(1);
});

test("the canvas states what to do before anything is traced", async ({ page }) => {
  await page.goto(LAB);
  await expect(page.locator(".viz-panel__empty")).toContainText("Nothing traced yet", {
    timeout: 30_000,
  });
  // The stdin box is the page's, shared by Run and Trace — it exists before any trace does.
  await expect(page.locator(".vlab__stdin-input")).toBeVisible();
});

test("the d2 export refuses to copy a figure that does not exist yet", async ({ page }) => {
  await page.goto(LAB);
  await expect(page.locator(".vlab__select option")).toHaveCount(17, { timeout: 30_000 });

  await page.locator(".vlab__export-btn").click();
  await expect(page.locator(".vlab__export-menu")).toBeVisible();
  await page.locator(".vlab__export-menu button", { hasText: "Copy this step" }).click();

  // An export with nothing traced must say so rather than putting an empty fence on the
  // clipboard — a silent copy is indistinguishable from a broken button.
  await expect(page.locator(".lab-toast")).toContainText("Trace something first");
});
