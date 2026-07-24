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
    // Coverage runs the ISLAND LOGIC — the node-testable half of web/ (markdown pipeline, lint,
    // diff, frontmatter, execution helpers). Preact `.tsx` islands mount in a browser and are
    // covered by the Playwright e2e suite instead, so they stay out of the denominator here —
    // counting them would report a number no unit test can move. Report-only (the CI gate is the
    // server crate, per ADR-RS004's testing note); `dev-tools/coverage.sh` runs this.
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov"],
      exclude: [
        "**/node_modules/**",
        "**/dist/**",
        "**/.astro/**",
        "**/*.astro",
        "**/*.gen.ts",
        "**/*.config.*",
        "src/lib/viz-wasm/**",
        "src/lib/islands/editor/monaco.ts", // Monaco is a browser editor — e2e territory
      ],
    },
  },
});
