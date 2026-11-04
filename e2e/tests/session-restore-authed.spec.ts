// A sign-in that survives quitting the browser. Keycloak's SSO cookie has no expiry for a GitHub
// login, so a browser restart loses it; the refresh token kept in localStorage picks the session
// back up. Deleting the context's cookies is that restart: localStorage stays, the cookie goes.
//
// The realm rotates refresh tokens and ends the session when one is spent too often, so these
// tests also pin the failure that design invites: two tabs knocking each other out.
//
// GATED (set E2E_AUTH=1) and run against a Keycloak-ALLOWLISTED origin: the dev server on :5373.
// `dev-tools/e2e-auth session-restore-authed` sets it up; the default `dev-tools/e2e` run skips
// this file. The dev Keycloak must carry the rotation settings in dev-tools/keycloak/synapse-realm.json
// (a realm file is imported on first start only).
import type { Page } from "@playwright/test";

import { expect, test } from "./fixtures";

const USER = process.env.E2E_KC_USER ?? "tester";
const PASS = process.env.E2E_KC_PASS ?? "tester";
const KEY = "auth-refresh-token";

const signedIn = (page: Page) => expect(page.locator(".account-chip__user")).toHaveText(`@${USER}`, { timeout: 30_000 });
const signedOut = (page: Page) => expect(page.locator(".account-chip__signin")).toBeVisible({ timeout: 30_000 });
const saved = (page: Page) => page.evaluate((key) => window.localStorage.getItem(key), KEY);

async function signIn(page: Page): Promise<void> {
  await page.goto("/");
  await page.locator(".account-chip__signin").click();
  await page.locator("#username").fill(USER);
  await page.locator("#password").fill(PASS);
  await page.locator("#kc-login").click();
  await signedIn(page);
  expect(await saved(page)).toBeTruthy();
}

async function signOut(page: Page): Promise<void> {
  await page.locator(".account-chip__user").click();
  await page.getByRole("button", { name: "Sign out" }).click();
}

/** What Keycloak says about a refresh token right now. Its issuer and client are in its own claims. */
async function refreshStatus(page: Page, token: string): Promise<number> {
  const claims = JSON.parse(Buffer.from(token.split(".")[1]!, "base64url").toString()) as { iss: string; azp: string };
  const response = await page.request.post(`${claims.iss}/protocol/openid-connect/token`, {
    form: { grant_type: "refresh_token", client_id: claims.azp, refresh_token: token },
  });
  return response.status();
}

test.describe("a saved session", () => {
  test.skip(!process.env.E2E_AUTH, "needs a real Keycloak: dev-tools/e2e-auth session-restore-authed");

  test("survives a browser restart, more than once", async ({ page, context }) => {
    await signIn(page);
    const first = await saved(page);

    await context.clearCookies();
    await page.reload();
    await signedIn(page);
    const second = await saved(page);
    expect(second).not.toBe(first);

    await context.clearCookies();
    await page.reload();
    await signedIn(page);
    expect(await saved(page)).not.toBe(second);
  });

  for (const cookie of ["after a restart", "while the SSO cookie lives"]) {
    test(`a tab holding an old token refreshes without ending the session, ${cookie}`, async ({ page, context }) => {
      await signIn(page);
      if (cookie === "after a restart") await context.clearCookies();

      // This tab signs in once and then sits on that token while another tab rotates past it.
      await page.clock.install();
      await page.reload();
      await signedIn(page);

      const other = await context.newPage();
      for (let visit = 0; visit < 3; visit++) {
        await other.goto("/");
        await signedIn(other);
      }
      const rotated = await saved(other);

      // Past the access token's 5-minute life, so the refresh loop has to spend a token.
      await page.clock.fastForward("06:00");
      await expect.poll(() => saved(other)).not.toBe(rotated);
      await signedIn(page);

      await other.reload();
      await signedIn(other);
    });
  }

  test("a token Keycloak rejects signs out and is dropped", async ({ page, context }) => {
    await signIn(page);
    await context.clearCookies();
    await page.evaluate((key) => window.localStorage.setItem(key, "not-a-refresh-token"), KEY);

    await page.reload();
    await signedOut(page);
    await expect.poll(() => saved(page)).toBeNull();
  });

  test("sign-out after a restart ends the session on the server", async ({ page, context }) => {
    await signIn(page);
    await context.clearCookies();
    await page.reload();
    await signedIn(page);
    const token = (await saved(page))!;
    expect(await refreshStatus(page, token)).toBe(200);

    await signOut(page);
    await signedOut(page);
    expect(await saved(page)).toBeNull();
    await page.reload();
    await signedOut(page);
    expect(await refreshStatus(page, token)).toBe(400);
  });

  test("signing out in one tab signs out the others", async ({ page, context }) => {
    await signIn(page);
    const other = await context.newPage();
    await other.goto("/");
    await signedIn(other);

    await signOut(page);
    await signedOut(other);
  });
});
