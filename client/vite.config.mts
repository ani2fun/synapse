import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// ─────────────────────────────────────────────────────────────────────────────
// VITE — dev server :5373, /api proxied to the server :8280. synapse-rs owns
// its OWN port pair (the Scala oracle holds 5273/8180) so both dev loops run
// side by side; strictPort keeps the origin STABLE — a silent bump to :5374
// would fall outside the Keycloak dev client's registered origins and break
// the silent-SSO iframe with a 403. The @alias map points at the TS islands;
// the wasm-bindgen glue's `import … from "@markdown/loader"` resolves through
// it, and each loader's dynamic import gives the heavy renderer its own
// chunk. vitest runs the island suites.
//
// A03 MOVED the markdown pipeline into the Astro app (web/src/lib/markdown),
// where the server-rendered lesson page now shares it — so `@markdown` points
// ACROSS the workspace at web/. Single-sourced there until A14 deletes this
// client; the wasm glue's `@markdown/loader` extern still resolves through the
// alias at build time, and render.test.ts now runs in web's vitest, not here.
// ─────────────────────────────────────────────────────────────────────────────

export default defineConfig({
  resolve: {
    alias: {
      "@markdown": fileURLToPath(new URL("../web/src/lib/markdown", import.meta.url)),
      "@editor": fileURLToPath(new URL("./islands/editor", import.meta.url)),
      "@auth": fileURLToPath(new URL("./islands/auth", import.meta.url)),
      "@tracer": fileURLToPath(new URL("./islands/tracer", import.meta.url)),
      "@diagram": fileURLToPath(new URL("./islands/diagram", import.meta.url)),
    },
  },
  server: {
    port: 5373,
    strictPort: true,
    proxy: {
      "/api": "http://localhost:8280",
      // LikeC4 lesson embeds (<iframe src="/c4/view/…">) ride the server's proxy.
      "/c4": "http://localhost:8280",
      "/media": "http://localhost:8280",
    },
  },
  build: {
    target: "esnext",
  },
  test: {
    // The island suites, plus the stylesheet-sanity gate (styles/) — a CSS file
    // that fails to parse drops rules silently in the browser, so it is checked
    // in CI rather than discovered by a reader.
    include: ["islands/**/*.test.ts", "styles/**/*.test.ts"],
  },
});
