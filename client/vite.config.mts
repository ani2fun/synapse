import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// ─────────────────────────────────────────────────────────────────────────────
// VITE — dev server :5273 (oracle convention), /api proxied to the server
// :8180. The @alias map points at the TS islands; the wasm-bindgen glue's
// `import … from "@markdown/loader"` resolves through it, and each loader's
// dynamic import gives the heavy renderer its own chunk. vitest runs the
// island suites (they live beside the islands in ../client-ts).
// ─────────────────────────────────────────────────────────────────────────────

export default defineConfig({
  resolve: {
    alias: {
      "@markdown": fileURLToPath(new URL("./islands/markdown", import.meta.url)),
    },
  },
  server: {
    port: 5273,
    proxy: {
      "/api": "http://localhost:8180",
    },
  },
  build: {
    target: "esnext",
  },
  test: {
    include: ["islands/**/*.test.ts"],
  },
});
