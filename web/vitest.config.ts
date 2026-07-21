import { defineConfig } from "vitest/config";

// ─────────────────────────────────────────────────────────────────────────────
// VITEST — the pure-logic suites under src/lib/ (A02: routes, seo; more join as their modules
// port). Plain node environment: nothing here touches the DOM or an Astro virtual module, so
// there is no need for `astro/config`'s `getViteConfig` wrapper — that can join when a suite
// actually needs it (an island component, `astro:content`, …).
// ─────────────────────────────────────────────────────────────────────────────

export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "styles/**/*.test.ts"],
  },
});
