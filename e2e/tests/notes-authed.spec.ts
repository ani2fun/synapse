// The signed-in half of the Notes tab: a note written here reaches the ACCOUNT, and comes back in
// a browser that has never seen it. That is the path the hermetic suite cannot reach — the note is
// stored in Postgres against a real sign-in, so nothing about sync exists without one.
//
// "Another machine" is simulated by deleting this browser's copy of the note from localStorage and
// reloading: whatever the pane then shows can only have come from `GET /api/notes`.
//
// GATED (set E2E_AUTH=1) and run against a Keycloak-ALLOWLISTED origin: the dev server on :5373 (a
// silent port bump 403s the silent-SSO iframe — the scar the repo records). `dev-tools/e2e-auth
// notes-authed` sets it up; the default `dev-tools/e2e` run skips this file.
//
// Self-cleaning: it clears the note at the start (a leftover from an interrupted run cannot be
// mistaken for this run's) and again at the end, and an empty save deletes the server row.
//
// NOT safe to run beside ITSELF: the note is one row per (account, problem), so two copies in
// parallel workers (`--repeat-each` without `--workers=1`) overwrite each other's text and fail.
// One copy per run — the normal case — shares that row with nothing.
import type { Page } from "@playwright/test";

import { expect, test } from "./fixtures";

const USER = process.env.E2E_KC_USER ?? "tester";
const PASS = process.env.E2E_KC_PASS ?? "tester";
const PROBLEM = process.env.E2E_AUTH_PROBLEM ?? "/synapse/learn/smoke/problems/threshold/threshold";
/** The pane's localStorage key: `problem-notes:<username>:<problem path>` (islands/notes/store). */
const KEY = `problem-notes:${USER}:${PROBLEM.replace(/^\/synapse\//, "")}`;

interface LocalNote {
  text: string;
  synced: boolean;
  base: string | null;
}

const readLocal = (page: Page) =>
  page.evaluate((key) => {
    const raw = window.localStorage.getItem(key);
    return raw === null ? null : (JSON.parse(raw) as LocalNote);
  }, KEY);

/** Wait until the account holds exactly `text`. The local copy's `synced` flag flips only after
 *  `PUT /api/notes` answered for that exact text, so it is a sharper signal than the status line,
 *  which reads "Synced" both before an edit and after it. An empty synced note removes the key. */
async function expectSynced(page: Page, text: string): Promise<void> {
  await expect
    .poll(async () => {
      const note = await readLocal(page);
      return text === "" ? note === null || (note.synced && note.text === "") : note?.synced === true && note.text === text;
    }, { timeout: 15_000 })
    .toBe(true);
  // An empty note that was never written reads "Syncs to your account" (nothing to sync yet), so
  // only a note with content has one status it must show.
  if (text !== "") await expect(page.locator(".pnotes__status")).toHaveText("Synced to your account");
}

/** Open the Notes tab once the page's island has wired it. The tab bar is server-rendered, so a
 *  click that lands before hydration hits a button with no listener and does nothing; the canvas
 *  rendering into the Think pane is the signal that the island is up. */
async function openNotes(page: Page): Promise<void> {
  await expect(page.locator(".pcanvas")).toBeVisible();
  await page.locator(".pwb__rtab--notes").click();
  await expect(page.locator(".pnotes")).toBeVisible();
}

/** Reload with this browser's copy gone, and open Notes — what a second machine would see. */
async function openFresh(page: Page): Promise<void> {
  await page.evaluate((key) => window.localStorage.removeItem(key), KEY);
  await page.reload();
  await expect(page.locator(".account-chip__user")).toHaveText(`@${USER}`, { timeout: 30_000 });
  await openNotes(page);
}

test.describe("signed-in notes — write, sync, read back elsewhere", () => {
  test.skip(
    !process.env.E2E_AUTH,
    "set E2E_AUTH=1 with Keycloak up and E2E_BASE_URL a realm-allowlisted origin (e.g. http://localhost:5373)",
  );

  test("sign in → write → the note follows the account → an unsent edit is pushed on open", async ({ page }) => {
    const marker = `e2e notes ${Date.now()}`;
    const field = page.locator(".pnotes__field");

    // ── sign in through Keycloak (keycloak-js redirects to the realm login form) ──
    await page.goto("/");
    await page.locator(".account-chip__signin").click();
    await page.locator("#username").fill(USER);
    await page.locator("#password").fill(PASS);
    await page.locator("#kc-login").click();
    await expect(page.locator(".account-chip__user")).toHaveText(`@${USER}`, { timeout: 30_000 });

    // ── the problem's Notes pane, starting from nothing ──
    await page.goto(PROBLEM);
    await openNotes(page);
    await expect(page.locator(".pnotes__status")).not.toHaveText("Syncing…");
    await field.fill("");
    await expectSynced(page, "");

    // ── write: the local copy lands first, then the account's ──
    await field.fill(`## ${marker}\n\nuse two pointers`);
    await expectSynced(page, `## ${marker}\n\nuse two pointers`);
    await expect(page.locator(".pnotes__count")).toHaveText(`${`## ${marker}\n\nuse two pointers`.length} / 10,000 characters`);

    // ── the toolbar writes markdown, and the edit syncs like typing does ──
    const start = `## ${marker}\n\nuse `.length;
    await field.evaluate((el: HTMLTextAreaElement, [from, to]) => el.setSelectionRange(from, to), [start, start + 3]);
    await page.locator(".pnotes__tool[aria-label^='Bold']").click();
    const bolded = `## ${marker}\n\nuse **two** pointers`;
    await expect(field).toHaveValue(bolded);
    await expectSynced(page, bolded);

    // ── another machine: nothing local, and the account's copy arrives ──
    await openFresh(page);
    await expect(field).toHaveValue(bolded);
    await expect(page.locator(".pnotes__status")).toHaveText("Synced to your account");

    // ── Preview renders it — through the notes pipeline, not the lesson one ──
    await page.locator(".pcanvas__seg-btn", { hasText: "Preview" }).click();
    await expect(page.locator(".pnotes__preview h2")).toHaveText(marker);
    await expect(page.locator(".pnotes__preview strong")).toHaveText("two");
    await page.locator(".pcanvas__seg-btn", { hasText: "Write" }).click();

    // ── an edit whose push never went out (the tab closed first) is sent on the next open ──
    // Written straight into storage as the pane itself would have: unsynced, based on the version
    // the server still holds. Opening the note must push it rather than replace it.
    const offline = `${bolded}\n\nedited offline`;
    await page.evaluate(
      ([key, text]) => {
        const note = JSON.parse(window.localStorage.getItem(key) ?? "null") as LocalNote;
        window.localStorage.setItem(key, JSON.stringify({ ...note, text, synced: false, savedAt: Date.now() }));
      },
      [KEY, offline] as const,
    );
    await page.reload();
    await expect(page.locator(".account-chip__user")).toHaveText(`@${USER}`, { timeout: 30_000 });
    await openNotes(page);
    await expect(field).toHaveValue(offline);
    await expectSynced(page, offline);

    // …and it really is on the server: a fresh browser reads it back.
    await openFresh(page);
    await expect(field).toHaveValue(offline);

    // ── clean up: an empty note deletes the row, so a fresh browser reads nothing ──
    await field.fill("");
    await expectSynced(page, "");
    await openFresh(page);
    await expect(field).toHaveValue("");
    await expect(page.locator(".pnotes__status")).toHaveText("Syncs to your account");
  });
});
