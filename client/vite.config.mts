import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// ─────────────────────────────────────────────────────────────────────────────
// VITE — dev server :5273 (oracle convention), /api proxied to the server
// :8180. The @alias map points at the TS islands; the wasm-bindgen glue's
// `import … from "@markdown/loader"` resolves through it, and each loader's
// dynamic import gives the heavy renderer its own chunk.
// ─────────────────────────────────────────────────────────────────────────────

export default defineConfig({
  resolve: {
    alias: {
      "@markdown": fileURLToPath(new URL("../client-ts/markdown", import.meta.url)),
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
});
