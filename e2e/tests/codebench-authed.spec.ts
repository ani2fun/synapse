// The signed-in codebench: edit a snippet, close it, come back, and still have the edit.
//
// This is the path the hermetic suite cannot reach — editing the codebench is gated on sign-in, so
// proving a restore needs a real Keycloak handshake and a writable Monaco. GATED (set E2E_AUTH=1)
// and run against a Keycloak-ALLOWLISTED origin, exactly like `authoring-authed.spec.ts`.
//
// `dev-tools/e2e-auth` filters to `authoring-authed` by default; extra args pass through, so:
//   dev-tools/e2e-auth codebench-authed
import { expect, test } from "./fixtures";

const USER = process.env.E2E_KC_USER ?? "tester";
const PASS = process.env.E2E_KC_PASS ?? "tester";
/** The fixture intro's plain ```python fence — runnable, so it grows a "Try in Editor" button. */
const LESSON = process.env.E2E_AUTH_LESSON ?? "/synapse/learn/smoke/intro";
/** The autosave debounce is 800 ms; wait past it before asserting anything reached storage. */
const PAST_DEBOUNCE_MS = 1_200;

test.describe("the codebench remembers an edited snippet", () => {
  test.skip(
    !process.env.E2E_AUTH,
    "set E2E_AUTH=1 with Keycloak up and E2E_BASE_URL a realm-allowlisted origin (e.g. http://localhost:5373)",
  );

  test("edit → Esc → reopen → reload keeps the buffer; Reset puts the fence back", async ({ page }) => {
    // ── sign in through Keycloak (keycloak-js redirects to the realm login form) ──
    await page.goto("/");
    await page.locator(".account-chip__signin").click();
    await page.locator("#username").fill(USER);
    await page.locator("#password").fill(PASS);
    await page.locator("#kc-login").click();
    await expect(page.locator(".account-chip__user")).toHaveText(`@${USER}`, { timeout: 30_000 });

    // Open state is a CLASS, asserted by count: `.codebench--open`'s children are both
    // `position: fixed`, so the wrapper measures 0px tall and `toBeVisible` would fail on a modal
    // that is plainly on screen. `.codebench__frame` is the element with a real box.
    const openBench = async () => {
      const tryBtn = page.locator(".fence-group__try").first();
      await expect(tryBtn).toBeVisible({ timeout: 30_000 });
      await tryBtn.click();
      await expect(page.locator(".codebench--open")).toHaveCount(1);
      const frame = page.locator(".codebench__frame");
      await expect(frame).toBeVisible();
      await expect(frame.locator(".view-lines")).toContainText("greet", { timeout: 30_000 });
      return frame;
    };

    await page.goto(LESSON);
    let frame = await openBench();
    // Signed in, so the buffer is writable and the sign-in notice is gone.
    await expect(page.locator(".codebench__signin")).toHaveCount(0);
    await expect(page.locator(".codebench__draft")).toHaveCount(0);

    // ── edit: place a real cursor, jump to the end, append a unique marker ──
    const marker = `# an edit from the e2e browser test, ${Date.now()}`;
    await frame.locator(".monaco-editor").click();
    await page.keyboard.press(process.platform === "darwin" ? "Meta+ArrowDown" : "Control+End");
    await page.keyboard.type(`\n${marker}`);

    // The draft bar appears the moment the buffer differs — it is also the only way back now.
    await expect(page.locator(".codebench__draft")).toBeVisible();
    await expect(page.locator(".codebench__draft")).toContainText(/line(s)? changed/);

    // ── stdin rides along in the same envelope ──
    const stdinBox = frame.locator(".codebench__stdin textarea");
    await stdinBox.fill("42\n");

    await page.waitForTimeout(PAST_DEBOUNCE_MS);
    const stored = await page.evaluate(() =>
      Object.keys(window.localStorage).filter((k) => k.startsWith("codebench-draft:")),
    );
    expect(stored).toHaveLength(1);
    // Keyed per account — a shared browser must not show this to the next reader.
    expect(stored[0]).toContain(`codebench-draft:${USER}:`);

    // ── Escape → reopen: the case the bug report is about ──
    await page.keyboard.press("Escape");
    await expect(page.locator(".codebench--open")).toHaveCount(0);
    frame = await openBench();
    await expect(frame.locator(".view-lines")).toContainText(marker);
    await expect(stdinBox).toHaveValue("42\n");

    // ── a full reload: the buffer lives in localStorage, not in the page ──
    await page.reload();
    frame = await openBench();
    await expect(frame.locator(".view-lines")).toContainText(marker);
    await expect(frame.locator(".codebench__stdin textarea")).toHaveValue("42\n");

    // ── Reset: back to the fence, draft bar gone, key dropped ──
    await page.locator(".codebench__draft .wb__ghost").click();
    await expect(frame.locator(".view-lines")).not.toContainText(marker);
    await expect(page.locator(".codebench__draft")).toHaveCount(0);
    await expect(frame.locator(".codebench__stdin textarea")).toHaveValue("");
    await expect
      .poll(() =>
        page.evaluate(
          () => Object.keys(window.localStorage).filter((k) => k.startsWith("codebench-draft:")).length,
        ),
      )
      .toBe(0);

    // ── and the reset survives a reopen, i.e. nothing stale was left behind ──
    await page.keyboard.press("Escape");
    frame = await openBench();
    await expect(frame.locator(".view-lines")).not.toContainText(marker);
  });
});
