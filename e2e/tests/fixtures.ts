import { test as base, expect } from "@playwright/test";

/**
 * Every spec imports `test` from here rather than from `@playwright/test` directly.
 *
 * The reason is a CI run that failed every hydration-dependent spec with `element(s) not
 * found` — the wasm app had not booted at all, and nothing in the output said so. A missing
 * `h1` and a missing `.header__search` are the *symptom*; the console error that killed the
 * boot is the cause, and it was invisible.
 *
 * This attaches to `pageerror` (uncaught exceptions, which is how a Rust panic surfaces in
 * wasm) and to `console.error`, then reports whatever it caught when a test fails. Two
 * consequences worth stating:
 *
 *  · A failure now prints the real reason next to the assertion that noticed it.
 *  · An uncaught page error FAILS the test even if the assertions somehow pass. A page that
 *    threw is not a page that works — this codebase has rendered something plausible while
 *    being fundamentally broken underneath, and assertions alone did not catch it.
 */
export const test = base.extend<{ pageErrors: string[] }>({
  pageErrors: [
    // eslint-disable-next-line no-empty-pattern
    async ({ page }, use, testInfo) => {
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));
      page.on("console", (message) => {
        if (message.type() === "error") errors.push(`console.error: ${message.text()}`);
      });

      await use(errors);

      if (errors.length === 0) return;
      const report = errors.map((e) => `  ${e.slice(0, 400)}`).join("\n");
      await testInfo.attach("page-errors", {
        body: errors.join("\n"),
        contentType: "text/plain",
      });

      // If the test ALREADY failed, print everything — including resource errors, which are
      // exactly what a failed wasm fetch looks like and are the most likely explanation for a
      // page that never booted. An attachment is no use when all you have is a CI log.
      if (testInfo.status !== testInfo.expectedStatus) {
        console.log(`\n  page errors during "${testInfo.title}":\n${report}\n`);
        return;
      }

      // The test passed, so only a real fault should overturn that. A page that threw is not a
      // page that works, even one that rendered something plausible while broken underneath.
      // Resource noise alone is not grounds to fail a passing spec.
      const real = errors.filter((e) => !/Failed to load resource/i.test(e));
      if (real.length > 0) {
        throw new Error(`the page reported errors:\n${report}`);
      }
    },
    { auto: true },
  ],
});

export { expect };
