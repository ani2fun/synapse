import { defineConfig, devices } from "@playwright/test";

/**
 * End-to-end smoke (step 52).
 *
 * These run against the PRODUCTION-SHAPED serve — the real axum server with `STATIC_ROOT`
 * pointing at a built `client/dist` — not against Vite. Two reasons, and the second is a scar:
 *
 *  1. It is the only way to exercise what the server actually does with a request: step 50's
 *     per-page `<title>` and meta injection, the cache headers, `/sitemap.xml`, `/robots.txt`.
 *     Vite serves `index.html` straight off disk and none of that exists in dev.
 *  2. Step 19's standing lesson, pinned in its chapter: dev NEVER reproduces CSP breakage,
 *     because Vite serves without the origin's security headers. A suite that only ever ran
 *     against Vite would be blind to the entire class of bug that has bitten this project
 *     hardest in production.
 *
 * The server is assumed to be already running (CI starts it as an explicit step, so its
 * readiness and its logs are visible rather than buried in a Playwright subprocess). Locally,
 * `dev-tools/e2e` builds the dist and starts it for you.
 */
const baseURL = process.env.E2E_BASE_URL ?? "http://localhost:8280";

export default defineConfig({
  testDir: "./tests",
  // A hydration-driven app has genuinely slow first paints (the wasm boots, then islands
  // mount). Generous per-assertion timeouts, but never a bare sleep in a spec.
  timeout: 60_000,
  expect: { timeout: 15_000 },
  // A flaky e2e suite is worse than none: people learn to re-run it, and then it protects
  // nothing. One retry in CI absorbs genuine infrastructure noise; a test that needs two is a
  // bug report.
  retries: process.env.CI ? 1 : 0,
  forbidOnly: !!process.env.CI,
  workers: process.env.CI ? 2 : undefined,
  reporter: process.env.CI ? [["list"], ["github"]] : [["list"]],
  use: {
    baseURL,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    // testIgnore is load-bearing: without it the desktop project also runs the phone
    // specs, which then fail for the right reason (the drawer is hidden above 1024px).
    { name: "chromium", use: { ...devices["Desktop Chrome"] }, testIgnore: /mobile\.spec\.ts/ },
    // Phone width is not a nicety here — steps 33, 42, 43 and 46 were all mobile-layout bugs,
    // and 46 shipped a 161px horizontal overflow that no desktop check could have seen.
    { name: "mobile", use: { ...devices["Pixel 5"] }, testMatch: /mobile\.spec\.ts/ },
  ],
});
