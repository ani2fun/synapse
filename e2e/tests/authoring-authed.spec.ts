// The full signed-in content-edit click-path, through a REAL Keycloak sign-in: land on a lesson,
// reveal and follow "Suggest an edit", change the markdown in Monaco, walk the three-step review
// dialog (Preview → Changes → Details), submit, and land on the dry-run confirmation.
//
// This is the one path the hermetic tests cannot reach — the browser, keycloak-js's PKCE
// handshake, the Monaco editor, and the live preview all together. It is GATED (set E2E_AUTH=1)
// and runs against a Keycloak-ALLOWLISTED origin: the dev server on :5373 (a silent port bump
// 403s the silent-SSO iframe — the same scar the repo records). `dev-tools/e2e-auth` sets it all
// up; the default `dev-tools/e2e` run skips this spec.
//
// The forge is dry-run (the deployment default), so submitting records the branch and opens no
// pull request — the flow is exercised end to end without a GitHub token, and re-runs are safe
// (a second submit reuses the open request).
import { expect, test } from "./fixtures";

const USER = process.env.E2E_KC_USER ?? "tester";
const PASS = process.env.E2E_KC_PASS ?? "tester";
// A PROSE lesson (not a problem page — the edit link is prose-only in v1). Defaults to the e2e
// FIXTURE's intro (`dev-tools/e2e-auth` points the server at fixture-content): stable, in-repo, and
// diagram-free, so the preview exercises the markdown pipeline without pulling extra lazy loaders.
// Override E2E_AUTH_LESSON (with SYNAPSE_ROOT) to drive a real content tree.
const LESSON = process.env.E2E_AUTH_LESSON ?? "/synapse/learn/smoke/intro";

test.describe("signed-in content edit — the full click-path", () => {
  test.skip(
    !process.env.E2E_AUTH,
    "set E2E_AUTH=1 with Keycloak up and E2E_BASE_URL a realm-allowlisted origin (e.g. http://localhost:5373)",
  );

  test("sign in → suggest an edit → review → submit (dry-run)", async ({ page }) => {
    // ── sign in through Keycloak (keycloak-js redirects to the realm login form) ──
    await page.goto("/");
    await page.locator(".account-chip__signin").click();
    await page.locator("#username").fill(USER);
    await page.locator("#password").fill(PASS);
    await page.locator("#kc-login").click();
    // Back on the app, the server-verified session is adopted — the chip shows the handle.
    await expect(page.locator(".account-chip__user")).toHaveText(`@${USER}`, { timeout: 30_000 });

    // ── the lesson reveals "Suggest an edit" for an allow-listed editor (hidden otherwise) ──
    await page.goto(LESSON);
    const editLink = page.locator("[data-edit-link]");
    await expect(editLink).toBeVisible({ timeout: 15_000 });
    await editLink.click();

    // ── /edit: Monaco mounts, then append a UNIQUE line at the END (prepending would break the
    //    frontmatter fence and the lint would block submission — which is the point of appending) ──
    await expect(page).toHaveURL(/\/edit\//);
    const editor = page.locator(".edit-page__editor");
    await expect(editor.locator(".monaco-editor")).toBeVisible({ timeout: 30_000 });
    // Wait for Monaco's MODEL to render its lines before editing — on a cold start the editor
    // element appears before the buffer is interactive, and keystrokes sent early are dropped or
    // misdirected (which then replaced the whole buffer and broke the frontmatter). Every lesson
    // source opens with its `title:` frontmatter, so this is content-agnostic.
    await expect(editor.locator(".view-lines")).toContainText("title:", { timeout: 30_000 });

    const marker = `An edit from the e2e browser test, ${Date.now()}.`;
    // Click to place a real cursor, jump to the END of the document, and append. Going to the end
    // (not select-all) means we can only ADD — never replace the buffer and lose the `---`
    // frontmatter fence. Monaco binds "cursor to bottom" to Cmd+Down on macOS, Ctrl+End elsewhere.
    await editor.locator(".monaco-editor").click();
    await page.keyboard.press(process.platform === "darwin" ? "Meta+ArrowDown" : "Control+End");
    await page.keyboard.type(`\n\n${marker}\n`);

    // "Review & submit" unlocks once the buffer is dirty — and must NOT read "(N to fix)", which
    // would mean a blocking lint error (e.g. a broken fence) is in the way.
    const review = page.locator(".edit-page__review");
    await expect(review).toBeEnabled();
    await expect(review).toHaveText(/^Review . submit$/);
    await review.click();

    // ── step 1 · Preview — the page rendered as a reader will see it, carrying the edit ──
    // 30s: the first preview lazily imports the whole markdown pipeline (unified + shiki), which a
    // cold dev server optimizes on first use. Instant once warm (a real dev session), slow once.
    await expect(page.locator(".edit-review__page")).toContainText(marker, { timeout: 30_000 });
    await page.locator(".edit-review__next").click();

    // ── step 2 · Changes — the line diff shows the addition ──
    await expect(page.locator(".edit-diff__row--added").filter({ hasText: marker })).toBeVisible();
    await page.locator(".edit-review__next").click();

    // ── step 3 · Details — a summary, then submit ──
    await page.locator("#edit-summary").fill("e2e verification: appended a sentence");
    await page.locator(".edit-review__submit").click();

    // ── done — dry-run records the per-user branch and opens no pull request ──
    await expect(page.locator(".edit-gate__title")).toContainText(/change (recorded|request)/i, { timeout: 30_000 });
    await expect(page.locator(".edit-gate__body")).toContainText(`edit/${USER}/`);
  });
});
