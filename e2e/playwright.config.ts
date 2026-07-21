import { defineConfig, devices } from "@playwright/test";

/**
 * The end-to-end smoke suite.
 *
 * These run against the PRODUCTION-SHAPED serve — the real axum server fronting the built
 * Astro SSR sidecar over `SYNAPSE_ASTRO_URL` (`web/dist/server/entry.mjs`) — not against Vite.
 * Two reasons:
 *
 *  1. It is the only way to exercise what the server actually does with a request: the
 *     per-page `<title>` and meta injection, the cache headers, `/sitemap.xml`, `/robots.txt`.
 *     Vite serves `index.html` straight off disk and none of that exists in dev.
 *  2. Dev NEVER reproduces CSP breakage, because Vite serves without the origin's security
 *     headers. A suite that only ever ran against Vite would be blind to the entire class of
 *     bug that has bitten this project hardest in production.
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
  // ONE worker in CI. A GitHub runner has ~4 GB shared with the Postgres service container,
  // and two workers across two projects meant several Chromium instances each instantiating a
  // multi-megabyte wasm module at once. The result was not a slow run but a dead one:
  //
  //     pageerror: WebAssembly.Table.grow(): failed to grow table by 4
  //
  // ...on every hydration-dependent spec, which then failed with a bare "element(s) not found"
  // because no component had mounted. Locally there is enough memory for it never to appear.
  //
  // Note for anyone tempted to raise this: an earlier version of this file set `workers: 1` for
  // a DIFFERENT reason (CPU contention) which turned out to be wrong, and it was reverted. This
  // time the reason is memory and the evidence is the error above.
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? [["list"], ["github"]] : [["list"]],
  use: {
    baseURL,
    launchOptions: {
      // `/dev/shm` is 64 MB in most CI containers and Chromium will happily exhaust it;
      // falling back to /tmp is the standard remedy. Harmless locally.
      args: ["--disable-dev-shm-usage"],
    },
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    // testIgnore is load-bearing: without it the desktop project also runs the phone
    // specs, which then fail for the right reason (the drawer is hidden above 1024px).
    { name: "chromium", use: { ...devices["Desktop Chrome"] }, testIgnore: /mobile\.spec\.ts/ },
    // Phone width is not a nicety here — this project has repeatedly shipped mobile-layout
    // bugs invisible at desktop width, including a 161px horizontal overflow that only a
    // phone-width check could have seen.
    { name: "mobile", use: { ...devices["Pixel 5"] }, testMatch: /mobile\.spec\.ts/ },
  ],
});
